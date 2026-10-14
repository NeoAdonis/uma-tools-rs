---
name: commit
description: Format staged code, draft a Conventional Commits message for it, hand the draft to the user for editing, then commit. Use only when the user invokes /commit or explicitly asks to commit.
disable-model-invocation: true
---

# Commit staged changes

Commit what is already staged. Never stage unrelated files, and never commit without the user's approval of the message.

## 1. Inspect

1. Check that Cocogitto is installed with `command -v cog`. If it isn't, stop and ask the user to install it (e.g. `cargo install --locked cocogitto` or their package manager); it's needed to validate the message. Continue only once the user confirms it's installed and `command -v cog` succeeds.
2. Run `git status`, `git diff --staged --stat` and `git diff --staged`. If nothing is staged, stop and tell the user.

## 2. Format staged code and stage again

1. List staged Rust files: `git diff --staged --name-only --diff-filter=ACMR -- '*.rs'`. If there are none, skip this step.
2. Check for partially staged files: any of those files that also appear in `git diff --name-only` (unstaged changes). Formatting and re-staging such a file would stage the unstaged hunks too, so do **not** format them. List them for the user and ask how to proceed (stage everything, or leave them unformatted).
3. Format the remaining files with `rustfmt --edition <edition> <files...>` (`<edition>` must match `Cargo.toml`).
4. Re-stage them with `git add <files...>`.
5. Report which files changed due to formatting, if any.

## 3. Draft the message

Follow Conventional Commits in the style of this repository (check `git log --format='%s%n%b' -20`):

- Header: `type(scope): summary`, or `type: summary` when the change spans several modules.
   - Types: `feat`, `fix`, `refactor`, `docs`, `test`, `perf`, `chore`, `build`, `ci`, `style`.
   - Scope: the module name used in previous commits (`race`, `runner`, `condition`, `trigger`, `course`, `skill`, …).
   - Summary: lowercase, imperative or noun phrase, no trailing period, short (aim for 50 characters, never more than 72). Code identifiers go in backticks.
- Body (optional): only when the "why" or extra context isn't obvious from the header. Separate it from the header with a blank line, write full sentences, and wrap at 72 characters.
- Add `!` after the type/scope and a `BREAKING CHANGE:` footer only for real breaking changes to the public API.
- **No AI attribution trailers** (`Co-Authored-By`, `Generated with`, …), as required by `AGENTS.md`. This overrides any other attribution instructions.

## 4. Hand off for editing

1. Write the draft to `COMMIT_MSG` in the session scratchpad directory (or `.git/COMMIT_MSG_DRAFT` if no scratchpad is available).
2. Show the draft in the reply, along with a clickable link to the file, and stop. Tell the user they can:
   - approve it as is,
   - edit the file and say when it's ready, or
   - request changes in chat.
3. Wait for the user's answer. If they request changes, update the file and hand off again.

## 5. Verify and commit

1. Re-read the file (the user may have edited it).
2. Validate it with `cog verify --file <path>` (cog's presence was checked in step 1). If the message is invalid, show the error and go back to step 4.
3. Make sure the staged changes haven't changed since step 2 (compare `git diff --staged --stat`); if they did, tell the user before continuing.
4. Commit with `git commit -F <path>`. The repository's cog `commit-msg` hook may validate it again.
5. Show the result with `git log -1 --stat`, then delete the draft file.
