# MCP Proxy Development Rules

This file defines the repository workflow for human contributors and coding agents. When any workflow guidance conflicts with `AGENTS.md` or `CLAUDE.md`, this file is the source of truth.

Repository-local workflow rules in this file also override any generic agent defaults, including default branch prefixes from external tooling or session-level instructions.

## Default Collaboration Model

- The canonical integration branch is `main`.
- Every change starts from the latest `main`.
- Do not commit directly to `main` unless the task is an emergency hotfix and the repo owner explicitly asks for it.
- Keep each branch focused on one change set. Unrelated fixes go into separate branches and separate PRs.

## Fork Rules

- If you have write access to `git@github.com:paderlol/mcp-proxy.git`, work from that repository directly and create a feature branch from `main`.
- If you do not have write access, fork `paderlol/mcp-proxy`, add your fork as your push remote, and open the PR back to `paderlol/mcp-proxy:main`.
- Do not fork a teammate's personal branch or fork unless the repo owner explicitly instructs you to collaborate there.
- Before opening a PR, make sure your branch is rebased on the latest upstream `main`.

## Which Branch To Use

- Use `main` only for syncing, local verification, and release/tag preparation approved by the repo owner.
- Use `feat/*` for user-visible features or new capabilities.
- Use `fix/*` for bug fixes or regressions.
- Use `refactor/*` for code cleanup without behavior changes.
- Use `docs/*` for documentation-only changes.
- Use `test/*` for test-only changes.
- Use `chore/*` for tooling, dependency, CI, or maintenance changes.
- Use `hotfix/*` only for urgent production-impacting fixes that need priority review and merge.

## Branch Naming Rules

- Format: `<type>/<scope>-<short-description>`
- Use lowercase letters, numbers, and hyphens only.
- Keep names short and specific. Prefer the affected module or user-facing area as the scope.
- Only the prefixes listed in `Which Branch To Use` are valid for this repository. Do not substitute other default prefixes such as `devops/`.
- Good examples:
  - `feat/docker-sandbox-launch`
  - `fix/config-export-windsurf`
  - `refactor/secrets-service-split`
  - `docs/add-branch-rules`
  - `test/cli-docker-e2e`
- Avoid vague names like `test`, `tmp`, `update`, `misc-fixes`, or `pader-work`.

## Branch Creation Process

1. Sync local `main` with the latest upstream.
2. Create a new branch from `main`.
3. Make only the scoped change for that branch.
4. Rebase onto the latest `main` before opening or updating the PR.

## Commit Rules

- Commit only coherent, reviewable units of work.
- Do not mix refactors, formatting churn, and functional changes in one commit unless they are inseparable.
- Run the smallest relevant test set before committing. If tests are skipped, say so in the PR description.
- Never commit secrets, generated local credentials, `.env` files, or machine-specific config.
- Prefer squash merging the PR unless the branch history is intentionally structured and easy to review.

## Commit Message Format

- Format: `<type>(<scope>): <summary>`
- Allowed types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `build`, `ci`, `perf`, `revert`
- Write the summary in imperative mood and keep it under about 72 characters.
- Good examples:
  - `feat(proxy): add trust warning before launch`
  - `fix(cli): preserve stdin after docker bootstrap`
  - `docs(rules): define branch and commit workflow`
  - `test(frontend): cover config export edge cases`

## Pull Request Rules

- One PR should solve one clear problem.
- PR title should follow the same format as the final squashed commit whenever possible.
- Include:
  - what changed
  - why it changed
  - how it was tested
  - any follow-up work or known limitations
- If the change affects UI or UX, include screenshots or a short recording.
- If the change affects security or secret handling, call that out explicitly in the PR description.

## Review And Merge Rules

- Do not self-merge until checks pass and review comments are addressed, unless the repo owner explicitly waives review.
- Resolve review comments with follow-up commits or a squash update; do not silently ignore them.
- Preserve a clean `main` branch. If a branch drifts or accumulates unrelated work, cut a fresh branch.

## Rules For Coding Agents

- Read `rules.md`, `AGENTS.md`, and `CLAUDE.md` before making workflow decisions.
- Unless instructed otherwise, branch from `main` and follow the naming rules in this file.
- If the current checkout is already on a task branch, continue there only when the work is clearly the same scope.
- If the workspace contains unrelated user changes, do not revert them; work around them or ask before touching them.
- When creating commits, use the commit message format in this file.
