# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-09-08

### Removed

- **BREAKING**: The Rust MCP server (`src/`, `Cargo.toml`, CI workflows, `commands/cc/`) is gone. The council pattern now ships as a single skill in `skills/council/SKILL.md` (#3). The server remains available at the `rust-legacy` and `v0.2.0` tags.
- The claudy MCP server dependency: councilors are reached through their own CLIs.

### Changed

- Dispatch channels are direct headless, read-only CLI invocations (`claude -p`, `codex exec`, `agy -p`, `grok -p`), so the skill runs on any agent host with a shell.

### Added

- OSS community files: CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, CHANGELOG, issue and pull request templates.
- Plugin packaging for Claude Code, Codex, Antigravity (agy), and Grok; install via each host's plugin manager (e.g. `claude plugin marketplace add epicsagas/council`).

## [0.2.0] - 2026-09-08

### Changed

- **BREAKING**: Adopt `llm-kernel` for transport, LLM client, and safety (#2).

## [0.1.2] - 2026-01-08

### Removed

- Deprecated `mcp.json.example`; README updated for MCP configuration.

## [0.1.1] - 2026-01-08

### Changed

- Version bump only.

## [0.1.0] - 2026-01-08

### Added

- Initial release of the Rust MCP council server.

[Unreleased]: https://github.com/epicsagas/council/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/epicsagas/council/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/epicsagas/council/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/epicsagas/council/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/epicsagas/council/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/epicsagas/council/releases/tag/v0.1.0
