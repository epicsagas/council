# Security Policy

## Supported versions

This project is a single Claude Code skill with no runtime dependencies. Only the latest `main` is supported.

## Reporting a vulnerability

Please use [GitHub private vulnerability reporting](https://github.com/epicsagas/mcp-council/security/advisories/new). Do not open a public issue for security reports.

## Scope

- **Skill prompts and procedure**: prompt-injection risks (a councilor's answer carrying instructions that manipulate the reviewer or chairman), anonymization leaks (model identity disclosed in Stage 2), and self-exclusion bypasses.
- **Artifacts**: `.council/` directories may contain sensitive discussion context. The skill instructs gitignoring them; reports about accidental inclusion are in scope.

Out of scope: the external CLIs (codex, agy) and the claudy MCP server itself; report issues in their own repositories.
