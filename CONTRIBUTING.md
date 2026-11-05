# Contributing

This is a personal project, developed at my own pace. Issues and pull requests
are still welcome, as long as you read the following first:

- [README.md](README.md): what the project does, and where it's going (see
  "Further goals").
- [AGENTS.md](AGENTS.md): the project overview for both humans and coding
  agents. It explains the design philosophy, code structure and commit rules.
- [STYLE.md](STYLE.md): conventions for formatting, linting, API design,
  testing, commit messages and terminology.

## Issues

- **Bugs**: include what you ran (configuration, Umas and race parameters, or an
  uma.moe build code), the seed (the demo prints its base seed), what happened,
  and what you expected to happen instead.
- **Differences from the game**: say where the expected behavior comes from
  (e.g. in-game observation, KuromiAK's write-up, game data).
- **Feature requests**: check the README's further goals first, and explain what
  you'd use the feature for.

## Pull requests

- Keep them focused on a single change. For large changes, open an issue first
  to discuss them.
- Before submitting, make sure these pass:
  - `cargo fmt --check`
  - `cargo clippy`
  - `cargo test`
  - `scripts/check_comment_width.pl`
- When adding or removing tests, follow the testing rules in
  [STYLE.md](STYLE.md#testing). There are a few optional tools that can help you
  with it:
  - `cargo llvm-cov --summary-only`, before and after removing tests, to find
    newly uncovered lines (use the same toolchain for both runs).
  - [`cargo mutants`](https://mutants.rs/) `--in-diff <diff>` (or `-f <file>`),
    to find code whose changes no test notices. It takes a few minutes per file.
- Write commit messages following [Conventional
  Commits](https://www.conventionalcommits.org/en/v1.0.0/), as described in
  [STYLE.md](STYLE.md).

Since I work on this project in my spare time, reviews may take a while. There's
no guarantee that every pull request will be merged: it might not fit the
project, or I might already be working on the same thing. If you're unsure, you
can always open an issue first to check before putting in the work. In any case,
thank you for your interest in this small project!
