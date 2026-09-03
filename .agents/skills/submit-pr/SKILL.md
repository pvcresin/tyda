---
name: submit-pr
description: Finish a change and open a pull request following this repo's conventions. Use when the user asks to commit, push, or create a PR.
---

# Submit a PR

## Before committing

1. Update the living doc matching your change — map in `AGENTS.md` (docs table).
2. Run `./scripts/check.sh` (fmt / clippy / all tests / release build). Docs-only
   changes may skip it (Markdown is not covered by formatters or tests).

## Commit

English, ≤50 chars, imperative verb first (`Add …`, `Fix …`, `Trim …`). One logical
change per commit.

## PR

- Title: English, imperative, like a commit subject.
- Body: English, following `.github/PULL_REQUEST_TEMPLATE.md` — keep the `## Summary`
  section (what/why, bullet points; subheadings per area for large PRs) and the
  `## Verification` section (check.sh checkbox plus any extra gates: render byte
  comparison, benchmark non-regression, testbed counts).
- Base branch: `main`.

```bash
gh pr create --title "<English imperative title>" --body-file <body.md>
```
