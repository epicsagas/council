//! Server-side LLM completion API built on llm-kernel.
//!
//! The council tools themselves are prompt/file coordinators: they return a
//! prompt that the *host* agent (Claude Code, Cursor) executes, which is the
//! product's interaction model. `complete_with` is the server-side escape
//! hatch for engines that must be driven directly over HTTP (Anthropic and
//! OpenAI-compatible providers from the llm-kernel catalog), with the
//! `cursor-agent` / `codex` CLI binaries as the fallback route. Every response
//! is secret-masked before it leaves this module, and every call is gated by
//! the optional `--max-tokens` budget.

use anyhow::{Context, Result};
use llm_kernel::llm::{AnthropicClient, LLMClient, LLMRequest, OpenAIClient};
use llm_kernel::provider::ProviderIndex;
use llm_kernel::safety::mask_secrets;
use llm_kernel::tokens::budget::TokenBudget;
use llm_kernel::tokens::estimate_tokens;
use std::sync::OnceLock;

const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-4-5";
/// Generation cap used for budget arithmetic when the caller did not pin one
/// (matches llm-kernel `ModelConfig`'s default).
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Result of one completion. `text` is already secret-masked.
pub struct CompletionOutput {
    pub text: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cost_usd: Option<f64>,
}

static BUDGET: OnceLock<Option<TokenBudget>> = OnceLock::new();

/// Install the deliberation-wide token budget (`--max-tokens`). Once set the
/// cap is fixed for the process lifetime; later calls are no-ops.
pub fn set_max_tokens_cap(cap: Option<u32>) {
    let _ = BUDGET.set(cap.map(TokenBudget::new));
}

/// Reserve `estimated + generation_cap` against the installed budget and
/// return the reserved amount (0 when no budget is installed). The caller
/// MUST settle via `settle_budget` on both success and failure paths, so
/// over-reserved tokens flow back instead of draining the budget.
fn reserve_budget(estimated_tokens: u32, generation_cap: u32) -> Result<u32> {
    match BUDGET.get().and_then(|b| b.as_ref()) {
        None => Ok(0),
        Some(budget) => {
            let need = estimated_tokens.saturating_add(generation_cap);
            if budget.try_reserve(need) {
                Ok(need)
            } else {
                Err(anyhow::anyhow!(
                    "Token budget exhausted: need {need} (prompt ~{estimated_tokens} + generation {generation_cap}), {} of {} remain. Raise --max-tokens or shorten the prompt.",
                    budget.remaining(),
                    budget.total()
                ))
            }
        }
    }
}

/// Return `reserved - actual_total` to the budget. No-op without a budget.
/// Actual usage is clamped to the reservation: a provider reporting more
/// prompt tokens than our estimate must not overdraw the budget on settle.
fn settle_budget(reserved: u32, actual_total: u32) {
    if reserved == 0 {
        return;
    }
    if let Some(Some(budget)) = BUDGET.get() {
        let refund = reserved.saturating_sub(actual_total.min(reserved));
        if refund > 0 {
            budget.release(refund);
        }
    }
}

enum Route {
    /// Direct HTTP via an llm-kernel client.
    Direct(Box<dyn LLMClient>),
    /// No direct API equivalent; shell out to the installed CLI.
    Cli(&'static str),
}

fn env_key(var: &str) -> Result<String> {
    std::env::var(var).context(format!(
        "{var} not set (mcp-council reads API keys from the environment only and never stores them)"
    ))
}

fn default_key_var(provider_id: &str) -> String {
    match provider_id {
        "openai" => "OPENAI_API_KEY".to_string(),
        "gemini" => "GEMINI_API_KEY".to_string(),
        other => format!("{}_API_KEY", other.to_uppercase().replace(['-', '.'], "_")),
    }
}

/// Map an engine name to either a direct llm-kernel client or the CLI fallback.
///
/// Accepted forms:
/// - `claude` / `anthropic`, optionally `claude:<model>` (ANTHROPIC_API_KEY)
/// - any embedded-catalog provider id (`gemini`, `zai`, `deepseek`, ...) or
///   exact model id (`glm-5`, `deepseek-chat`, ...) resolved via `ProviderIndex`
/// - `cursor-agent` / `codex` keep the CLI fallback path
fn resolve(engine: &str) -> Result<Route> {
    let engine = engine.trim();
    if engine.is_empty() {
        anyhow::bail!(
            "Empty engine name. Use 'claude', a catalog provider/model id, or 'cursor-agent'/'codex'."
        );
    }

    // Anthropic is absent from the embedded catalog, so it is wired directly.
    let claude_model = engine
        .strip_prefix("claude:")
        .or_else(|| engine.strip_prefix("anthropic:"))
        .map(str::to_string)
        .or_else(|| {
            (engine == "claude" || engine == "anthropic").then(|| DEFAULT_CLAUDE_MODEL.to_string())
        });
    if let Some(model) = claude_model {
        let key = env_key("ANTHROPIC_API_KEY")?;
        let client = AnthropicClient::from_key(model, key).map_err(anyhow::Error::from)?;
        return Ok(Route::Direct(Box::new(client)));
    }

    if engine == "cursor-agent" || engine == "codex" {
        return Ok(Route::Cli(if engine == "codex" {
            "codex-cli"
        } else {
            "cursor-agent"
        }));
    }

    let index = ProviderIndex::embedded();
    let service = index
        .get(engine)
        .or_else(|| index.find_model(engine).map(|(s, _)| s))
        .with_context(|| {
            format!(
                "Unknown engine '{engine}'. Use 'claude[:<model>]', a provider id from the llm-kernel catalog (gemini, zai, deepseek, ...), an exact model id, or 'cursor-agent'/'codex'."
            )
        })?;
    let model_id = index
        .find_model(engine)
        .map(|(_, m)| m.id.clone())
        .unwrap_or_else(|| service.default_model.clone());

    let key_var = if service.key_var.is_empty() {
        default_key_var(&service.id)
    } else {
        service.key_var.clone()
    };
    let key = env_key(&key_var)?;
    let base_url = service
        .api_base_url
        .clone()
        .with_context(|| format!("Catalog entry '{}' has no api_base_url", service.id))?;

    let client =
        OpenAIClient::from_key_with_base_url(model_id, key, base_url, reqwest::Client::new());
    Ok(Route::Direct(Box::new(client)))
}

/// Run one completion through the resolved route.
///
/// The response text is passed through `safety::mask_secrets` before it leaves
/// this function, and estimated cost is logged to stderr when the catalog has
/// pricing for the model.
pub async fn complete_with(
    engine: &str,
    prompt: &str,
    max_tokens: Option<u32>,
) -> Result<CompletionOutput> {
    let estimated = estimate_tokens(prompt) as u32;
    let generation_cap = max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    let reserved = reserve_budget(estimated, generation_cap)?;
    // From here on, every exit path must settle the reservation, otherwise a
    // failed deliberation permanently drains the budget.
    let result = complete_inner(engine, prompt, max_tokens, estimated, generation_cap).await;
    match &result {
        Ok(out) => settle_budget(
            reserved,
            out.prompt_tokens.saturating_add(out.completion_tokens),
        ),
        Err(_) => settle_budget(reserved, 0),
    }
    result
}

async fn complete_inner(
    engine: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    estimated: u32,
    generation_cap: u32,
) -> Result<CompletionOutput> {
    eprintln!("mcp-council: engine={engine} ~{estimated} prompt tokens, cap={generation_cap}");

    match resolve(engine)? {
        Route::Cli(bin) => {
            let raw = crate::cli_runner::run_llm(bin, prompt).await?;
            let text = mask_secrets(&llm_kernel::safety::strip_ansi(&raw));
            // CLI engines do not report usage; estimate from the output so
            // budget accounting does not silently under-count.
            let output_tokens = estimate_tokens(&text) as u32;
            Ok(CompletionOutput {
                text,
                prompt_tokens: estimated,
                completion_tokens: output_tokens,
                cost_usd: None,
            })
        }
        Route::Direct(client) => {
            let request = LLMRequest {
                messages: vec![llm_kernel::llm::ChatMessage::user(prompt)],
                max_tokens,
                ..LLMRequest::default()
            };
            let response = client.complete(request).await?;
            let model_id = client.model_name().to_string();
            let cost = ProviderIndex::embedded().estimate_cost(
                &model_id,
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
            );
            if let Some(cost) = cost {
                eprintln!(
                    "mcp-council: model={model_id} in={} out={} est. ${cost:.6}",
                    response.usage.prompt_tokens, response.usage.completion_tokens
                );
            }
            Ok(CompletionOutput {
                text: mask_secrets(&response.content),
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                cost_usd: cost,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_err(res: Result<Route>) -> String {
        match res {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn budget_gates_and_settles() {
        // No budget installed: reservations are free (0) and settling is a no-op.
        assert_eq!(reserve_budget(1_000_000, DEFAULT_MAX_TOKENS).unwrap(), 0);
        // Once the cap is installed, overflow is rejected. BUDGET is
        // process-global, so all assertions live in this one test.
        let _ = BUDGET.set(Some(TokenBudget::new(1000)));
        assert!(reserve_budget(800, DEFAULT_MAX_TOKENS).is_err());
        // A successful reservation is returned and settles back to the cap.
        let reserved = reserve_budget(100, 800).unwrap();
        assert_eq!(reserved, 900);
        settle_budget(reserved, 300); // refund 600, 300 stays spent
        assert!(reserve_budget(300, 400).is_ok()); // exactly the remaining 700
    }

    #[test]
    fn empty_engine_is_rejected() {
        assert!(resolve("").is_err());
        assert!(resolve("  ").is_err());
    }

    #[test]
    fn anthropic_models_route_without_catalog() {
        // claude resolves via AnthropicClient, not the catalog. Key presence
        // decides between a direct route and the env-var error; either way the
        // engine name must be known.
        match resolve("claude") {
            Ok(Route::Direct(_)) => {}
            Ok(Route::Cli(_)) => panic!("claude must not route to CLI"),
            Err(e) => assert!(e.to_string().contains("ANTHROPIC_API_KEY"), "{e}"),
        }
    }

    #[test]
    fn unknown_engine_lists_alternatives() {
        let err = expect_err(resolve("definitely-not-an-engine"));
        assert!(err.contains("Unknown engine"), "{err}");
    }

    #[test]
    fn cli_engines_route_to_cli() {
        assert!(matches!(
            resolve("cursor-agent"),
            Ok(Route::Cli("cursor-agent"))
        ));
        assert!(matches!(resolve("codex"), Ok(Route::Cli("codex-cli"))));
    }

    #[test]
    fn catalog_provider_resolves_base_url_and_key_var() {
        // deepseek carries key_var DEEPSEEK_API_KEY and api_base_url in the
        // catalog. Without the key set we must get the key error, proving the
        // catalog lookup got that far.
        if std::env::var("DEEPSEEK_API_KEY").is_ok() {
            assert!(matches!(resolve("deepseek"), Ok(Route::Direct(_))));
        } else {
            let err = expect_err(resolve("deepseek"));
            assert!(err.contains("DEEPSEEK_API_KEY"), "{err}");
        }
    }

    #[test]
    fn exact_model_id_resolves_provider() {
        // glm-5 exists as a model id in the embedded catalog.
        if std::env::var("ZAI_API_KEY").is_ok() {
            assert!(matches!(resolve("glm-5"), Ok(Route::Direct(_))));
        } else {
            let err = expect_err(resolve("glm-5"));
            assert!(err.contains("ZAI_API_KEY"), "{err}");
        }
    }
}
