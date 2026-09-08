---
name: council
description: Multi-model council (claude + codex + agy). Independent answers, anonymized cross peer-review with self-exclusion, then chairman synthesis. Use for architecture decisions, contested technical questions, and high-stakes judgments where a genuinely independent second opinion matters. Invoke as "/council <question>".
---

# Council

Three-stage deliberation: independent answers from multiple model families, anonymized peer review among them, synthesis by a chairman. Pattern ported from the mcp-council Rust MCP server (see the `rust-legacy` git tag); the plumbing is now native agent tooling, no MCP server required.

## Panel

Default councilors, dispatched in parallel from a single message:

| Councilor | Channel |
|---|---|
| claude | Agent tool, subagent `general-purpose` |
| codex | `mcp__claudy__ask_agent` with `agent: "codex"` |
| agy (Gemini) | `mcp__claudy__ask_agent` with `agent: "agy"` |

If the user names a panel, use theirs. Otherwise use the default three.

## Procedure

### 0. Setup

1. Question = the user's argument. If empty, ask for the question first.
2. Slug: lowercase the question, spaces to `-`, keep only `[a-z0-9-]`, trim and cap at 40 chars.
3. Workspace: `.council/<slug>/` in the current project. If it exists, append `-2`, `-3`, ... Create the directory.

### 1. Stage 1 — Independent answers

Dispatch all councilors in ONE message (parallel). Each receives the question verbatim and nothing from the other councilors. Councilor prompt template:

```
Answer the following question thoroughly and independently. You may use tools to inspect the project as needed, but your final message must be the complete, self-contained answer. Do not mention your identity, model name, or vendor anywhere in the answer.

Question:
<question>
```

Save each reply as `.council/<slug>/<model>-answer.md` with this header:

```markdown
# <model> answer

- date: <YYYY-MM-DD>
- question: <question>

---

<reply verbatim>
```

The main session performs ALL file writes. Councilors never write council files.

### 2. Anonymize

Build the label map in memory: surviving answers become `Response A`, `Response B`, ... in dispatch order. Reviewer prompts contain labels only, never model names.

### 3. Stage 2 — Peer review (self-exclusion)

For each councilor, dispatch (parallel, same channel it used in Stage 1) the review prompt: the anonymized packet with that councilor's own response REMOVED. After removal, relabel the remaining responses so labels stay consecutive. Each Stage 2 dispatch must open a fresh session (claudy `ask_agent`); never reuse a Stage 1 session via `send_message`, or the reviewer could see its own Stage 1 answer and self-exclusion is defeated.

```
You are evaluating different responses to the following question:

Question: <question>

Here are the responses from different models (anonymized):

<Response A>: ...
<Response B>: ...

Your task:
1. First, evaluate each response individually. For each response, explain what it does well and what it does poorly.
2. Then, at the very end of your response, provide a final ranking.

IMPORTANT: Your final ranking MUST be formatted EXACTLY as follows:
- Start with the line "FINAL RANKING:" (all caps, with colon)
- Then list the responses from best to worst as a numbered list
- Each line should be: number, period, space, then ONLY the response label (e.g., "1. Response A")
- Do not add any other text or explanations in the ranking section

Do not speculate about which model produced each response; judge the content only.
```

Save each review verbatim as `.council/<slug>/peer-review-by-<model>.md`. No trimming, no cleanup.

### 4. Stage 3 — Chairman synthesis

The current session acts as chairman; no dispatch. Apply the chairman prompt to the full record (de-anonymized):

> You are the Chairman of an LLM Council. Multiple AI models answered a question, then ranked each other's responses. Synthesize everything into a single, comprehensive, accurate final answer. Consider: the individual responses and their insights, the peer rankings and what they reveal about response quality, and patterns of agreement or disagreement.

Write the synthesis to `.council/<slug>/final-answer.md`.

### 5. Report

Reply in chat: the verdict in 3 to 5 lines (chairman's bottom line, strongest consensus, sharpest disagreement), then the workspace file list.

## Degradation

Log every failure (backend error, empty reply) to `.council/<slug>/run-log.md` and drop that councilor.

- 2+ survivors: normal flow.
- 1 survivor: skip Stage 2. The chairman critically reviews the single answer itself (strengths, gaps, corrections) before synthesizing; note the degraded mode in `final-answer.md` and the report.
- 0 survivors: report the backend errors, write nothing else.

If the `claudy` MCP server is entirely unavailable, only `claude` remains; apply the 1-survivor rule.

## Rules

- Stage 1 councilors never see each other's answers. Independence is the whole point.
- The reviewer's own response is always excluded from its review packet.
- Model names never appear in Stage 2 prompts. Labels only.
- Reviews and answers are stored verbatim. The chairman may disagree with rankings but may not edit them.
- All council artifacts live in `.council/<slug>/`. The project's `.gitignore` should contain `.council/`.
