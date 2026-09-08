use anyhow::Result;
use llm_kernel::error::KernelError;
use llm_kernel::mcp::{JsonRpcDispatcher, McpServer as KernelMcpServer, ToolDescription};
use serde_json::{Value, json};

fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "council.first_answer",
            "Stage1: Save current model answer into .council/{slug}/{model}-answer.md",
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Conversation title/directory name (slug)"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model name (e.g., sonnet, gemini, gpt-5.1)",
                        "default": "unknown-model"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "User question or prompt text"
                    },
                    "content": {
                        "type": "string",
                        "description": "Full model answer content to save"
                    }
                },
                "required": ["title", "prompt", "content"]
            }),
        ),
        (
            "council.peer_review",
            "Stage2: Read Stage1 JSON files and generate peer review using local LLM CLI",
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Conversation title/directory name"
                    },
                    "model": {
                        "type": "string",
                        "description": "LLM model name performing the review (examples: claude, gemini, glm-4.6)",
                        "default": "claude"
                    },
                    "self_model": {
                        "type": "string",
                        "description": "Model name to exclude from peer review (its own response)"
                    }
                },
                "required": ["title"]
            }),
        ),
        (
            "council.finalize",
            "Stage3: Read Stage1 and Stage2 JSON files and generate final answer using local LLM CLI",
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Conversation title/directory name"
                    },
                    "model": {
                        "type": "string",
                        "description": "LLM model name performing the synthesis (examples: claude, gemini, glm-4.6)",
                        "default": "claude"
                    },
                    "engine": {
                        "type": "string",
                        "description": "LLM model/engine (for backward compatibility, use 'model' instead)",
                        "default": "claude"
                    }
                },
                "required": ["title"]
            }),
        ),
        (
            "council.save_review",
            "Save peer review content to markdown file",
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Conversation title/directory name"
                    },
                    "model": {
                        "type": "string",
                        "description": "LLM model name (examples: claude, gemini, glm-4.6, gpt-4)"
                    },
                    "engine": {
                        "type": "string",
                        "description": "LLM model/engine name (for backward compatibility, use 'model' instead)",
                        "default": "claude"
                    },
                    "content": {
                        "type": "string",
                        "description": "Peer review content to save"
                    }
                },
                "required": ["title", "content"]
            }),
        ),
        (
            "council.summarize",
            "Generate a summary prompt for large documents to reduce token costs in Stage2/Stage3",
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Conversation title/directory name (slug)"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model name performing the summary (e.g., sonnet, gemini, gpt-5.1)",
                        "default": "unknown-model"
                    },
                    "content": {
                        "type": "string",
                        "description": "Original content to summarize"
                    },
                    "max_length": {
                        "type": "integer",
                        "description": "Target summary length in characters (default: 2000)",
                        "default": 2000
                    }
                },
                "required": ["title", "content"]
            }),
        ),
        (
            "council.save_summary",
            "Save summary content to markdown file for use in Stage2/Stage3",
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Conversation title/directory name"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model name that generated the summary"
                    },
                    "content": {
                        "type": "string",
                        "description": "Summary content to save"
                    }
                },
                "required": ["title", "content"]
            }),
        ),
    ]
}

fn into_kernel_result(result: Result<Value>) -> llm_kernel::error::Result<Value> {
    // Tool handlers are anyhow-based; KernelError has no anyhow variant, so the
    // message travels in Config.
    result.map_err(|e| KernelError::Config(e.to_string()))
}

pub struct McpServer {
    kernel: KernelMcpServer,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        let mut kernel = KernelMcpServer::new("mcp-council", env!("CARGO_PKG_VERSION"));

        for (name, description, input_schema) in tool_specs() {
            kernel.register_tool(ToolDescription {
                name: (*name).to_string(),
                description: (*description).to_string(),
                input_schema: input_schema.clone(),
            });
        }

        kernel.set_async_handler("council.first_answer", |params| async move {
            into_kernel_result(crate::tools::first_answer::handle_first_answer(params).await)
        });
        kernel.set_async_handler("council.peer_review", |params| async move {
            into_kernel_result(crate::tools::peer_review::handle_peer_review(params).await)
        });
        kernel.set_async_handler("council.finalize", |params| async move {
            into_kernel_result(crate::tools::finalize::handle_finalize(params).await)
        });
        kernel.set_async_handler("council.save_review", |params| async move {
            into_kernel_result(crate::tools::save_review::handle_save_review(params).await)
        });
        kernel.set_async_handler("council.summarize", |params| async move {
            into_kernel_result(crate::tools::summarize::handle_summarize(params).await)
        });
        kernel.set_async_handler("council.save_summary", |params| async move {
            into_kernel_result(crate::tools::save_summary::handle_save_summary(params).await)
        });

        Self { kernel }
    }

    pub async fn run(&self) -> Result<()> {
        let dispatcher = JsonRpcDispatcher::new(&self.kernel);
        dispatcher
            .run_stdio_async()
            .await
            .map_err(|e| anyhow::anyhow!("stdio transport failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(fut)
    }

    fn dispatch(server: &KernelMcpServer, line: &str) -> Option<String> {
        block_on(JsonRpcDispatcher::new(server).dispatch_async(line))
    }

    #[test]
    fn initialize_lists_server_info() {
        let server = McpServer::new().kernel;
        let resp = dispatch(
            &server,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        )
        .unwrap();
        assert!(resp.contains("\"name\":\"mcp-council\""), "{resp}");
    }

    #[test]
    fn tools_list_has_all_six_tools() {
        let server = McpServer::new().kernel;
        let resp = dispatch(&server, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        for name in [
            "council.first_answer",
            "council.peer_review",
            "council.finalize",
            "council.save_review",
            "council.summarize",
            "council.save_summary",
        ] {
            assert!(resp.contains(name), "missing {name} in {resp}");
        }
    }

    #[test]
    fn unknown_method_is_32601() {
        let server = McpServer::new().kernel;
        let resp = dispatch(
            &server,
            r#"{"jsonrpc":"2.0","id":3,"method":"nope/method"}"#,
        )
        .unwrap();
        assert!(resp.contains("-32601"), "{resp}");
    }

    #[test]
    fn unknown_tool_is_32602() {
        let server = McpServer::new().kernel;
        let resp = dispatch(
            &server,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"council.nope","arguments":{}}}"#,
        )
        .unwrap();
        assert!(resp.contains("-32602"), "{resp}");
    }

    #[test]
    fn notification_gets_no_response() {
        let server = McpServer::new().kernel;
        let resp = dispatch(
            &server,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert!(resp.is_none());
    }

    #[test]
    fn tool_error_is_in_band_is_error() {
        // save_summary against a nonexistent council dir must come back as
        // isError:true inside a successful JSON-RPC envelope, per MCP spec.
        let server = McpServer::new().kernel;
        let resp = dispatch(
            &server,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"council.save_summary","arguments":{"title":"definitely-missing-dir-xyz","content":"x"}}}"#,
        )
        .unwrap();
        assert!(resp.contains("\"isError\":true"), "{resp}");
    }
}
