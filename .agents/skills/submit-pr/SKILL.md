---
name: submit-pr
description: Finish a change and open a pull request following this repo's conventions. Use when the user asks to commit, push, or create a PR.
---

# Submit a PR

## Before committing

1. Update the living doc matching your change — map in `AGENTS.md` (docs table).
2. Run `pr-self-review` on the complete diff and resolve its findings.
3. Run `mise run check` for implementation changes. For docs-only changes, run
   relevant docs checks. Always run `git diff --check`.

## Commit

English, ≤50 chars, imperative verb first (`Add …`, `Fix …`, `Trim …`). One logical
change per commit.

## PR

- Title: English, imperative, like a commit subject.
- Body: English, following `.github/PULL_REQUEST_TEMPLATE.md` — keep its
  `## Background`, `## Implementation`, and `## Test` sections concise. Include
  scenario and all-subject benchmark results when relevant.
- Base branch: `main`.

```bash
gh pr create --title "<English imperative title>" --body-file <body.md>
```
