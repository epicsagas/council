# Contributing

Thanks for your interest in improving Council. The project is intentionally small: a single skill file (`skills/council/SKILL.md`), a README, and community files. Keeping it small is a feature.

## How to propose a change

1. Open an issue first if the change alters the council procedure (Stages 1-3, anonymization, self-exclusion, degradation rules). Procedure changes benefit from debate before code.
2. Fork, create a branch, and keep the diff minimal.
3. Commit with [Conventional Commits](https://www.conventionalcommits.org) (`feat:`, `fix:`, `docs:`, `chore:`).
4. Open a pull request with a short description of what changed and why.

## Testing your change

There is no build. The test is running the skill:

1. Copy `skills/council/` into `~/.claude/skills/` (or symlink it).
2. Run `/council <a real question you care about>` in a scratch project.
3. Check `.council/<slug>/`: all answers and peer reviews saved verbatim, `final-answer.md` synthesized, rankings parse, self-exclusion held (no councilor reviewed its own answer).
4. If you changed the prompt templates, verify the `FINAL RANKING:` output format still parses.

## Scope guidelines

- Changes to `SKILL.md` prompts and procedure: welcome, with evidence from a real run.
- Adding new councilor backends: welcome if they follow the existing channel pattern (parallel dispatch, self-exclusion via fresh session).
- New features beyond the pattern (UIs, servers, configs): propose in an issue first. The answer is often "the agents can already do that".

## Code of Conduct

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
