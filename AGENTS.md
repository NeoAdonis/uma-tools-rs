# Project overview for coding agents (and curious humans)

This project contains a Rust library with functions and data structures aimed at simulating Umamusume: Pretty Derby races at scale in order to analyze performance metrics and strategies, plus a small binary to demonstrate it.

## Design philosophy

- Worse is better: Prioritize simplicity and ease of implementation over complexity and completeness.
- DRY (Don't Repeat Yourself) and AHA (Avoid Hasty Abstractions): Avoid redundancy by reusing code wherever possible without adding unnecessary abstraction.
- Performance: Optimize for speed and efficiency to handle large-scale simulations effectively.
- Testing: Implement thorough testing to ensure correctness and reliability of the simulations and analyses. Use `cargo llvm-cov` as you deem fit to check coverage.
- Refactor freely: the project is early-stage with no backward-compatibility guarantees, so prefer a clean reimplementation over patching, as long as it respects the principles above.
- Code as the source of truth: The code, including its doc comments, is its own primary documentation; keep doc comments up to date when changing it. If there are any inconsistencies between other documentation and the code, the code should be considered the source of truth.

## Coding conventions

Follow the style conventions in [`STYLE.md`](./STYLE.md), especially for code comments. In short:

- The project uses the default Rust style, so `cargo fmt` (with the appropriate flags, e.g. `--check`) can both fix and verify formatting.
- Use `cargo clippy` to check for common mistakes and non-idiomatic code.
- Prefer line comments (`//`, `///`, `//!`).
- Comment lines are at most 80 characters long (sigils included, indentation excluded), without exceeding the global line limit. Run `scripts/check_comment_width.pl` to verify it.
- Follow the API design rules in [`STYLE.md`](./STYLE.md), based on the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), when changing the public API.

## API design decisions

The following deviations from the Rust API Guidelines are known and currently accepted. They aren't final and may be revisited, but don't change them as part of unrelated work:

- `CompEvaluator` isn't sealed ([C-SEALED](https://rust-lang.github.io/api-guidelines/future-proofing.html#c-sealed)), since its docs invite other crates to implement it.
- Some types are built from a number with named constructors (`from_meters`, `from_delay`) rather than `From` impls ([C-CONV-TRAITS](https://rust-lang.github.io/api-guidelines/interoperability.html#c-conv-traits)), because a bare number doesn't say what it measures.
- Types don't derive `Serialize` ([C-SERDE](https://rust-lang.github.io/api-guidelines/interoperability.html#c-serde)). Add it only when results need to be exported.
- Some public enums derive `TryFrom` through `num_enum`, which exposes its `TryFromPrimitiveError` in the public API ([C-STABLE](https://rust-lang.github.io/api-guidelines/necessities.html#c-stable)).

## Commits

- Coding agents shouldn't create commits on their own, only when explicitly asked by the user.
- Follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification and the 50/72 rule, as described in [`STYLE.md`](./STYLE.md).
- If possible, use a tool such as Cocogitto (`cog check`, `cog verify`) to validate commit messages.
- Don't add AI attribution trailers (e.g. "Co-Authored-By", "Generated with") to commits or PR descriptions; they clutter permanent history and can read as product promotion. This isn't opposition to transparency about AI use.

## Code structure

Source code, relative to `src/`:

- `lib.rs`: library root; declares modules and re-exports commonly used types.
- `configuration.rs`: simulation behavior, game version and language settings.
- `condition.rs`, `condition/`: condition parsing and evaluation.
- `course.rs`, `course/`: course properties and calculations.
- `race.rs`, `race/`: race properties, mechanics and simulation.
  - `race/solver.rs`: race initialization and simulation.
  - `race/unready_runner.rs`: runner properties and calculations prior to simulation.
  - `race/runner.rs`, `race/runner/`: runner properties and calculations during simulation.
  - `race/hp.rs`: HP calculations.
  - `race/popularity.rs`: runner popularity calculations.
  - `race/transitions.rs`: calculations to detect transitions between race sections and course features.
  - `race/rng.rs`: random number streams, derived per runner, mechanic and skill from the race seed.
  - Any other module in `race/` contains properties and calculations related to a specific race mechanic.
- `trigger.rs`, `trigger/`: trigger system to activate skills based upon their conditions and variables. The `trigger` module docs give an overview of the whole skill activation system.
- `skill.rs`: skill data structures and helpers.
- `import.rs`, `import/`: import of Umas from data exported by other tools (e.g. `import/umaextractor.rs` for UmaExtractor).
- `uma.rs`: Umamusume data structures and helpers.
- `names.rs`: character, skill and track names in Japanese and English, embedded from `data/names.json`.
- `units.rs`: conversion helpers between many units of measure used in the game and data.
- `test_utils.rs`: reusable helpers to create data structures for unit tests.
- `main.rs`: small application to demonstrate race simulations.
- `bin/multi_race.rs`: small application to simulate the same race many times
  with different seeds and summarize finish times, also used for profiling.

Test code, relative to `tests/`:

- `integration/`: integration tests.
  - `integration/common.rs`: common test code and helpers to create data structures for integration tests.
- `data/`: data files used by tests and doc examples (e.g. `umaextractor_uma.json`, an Uma exported by UmaExtractor; `uma_data.json`, an Uma in the `UmaData` format).

Documentation, relative to `docs/`:

- `CONDITIONS.md`: conditions and variables used to determine skill activation.
- `ADDING_CONDITION_VARIABLES.md`: steps to support a new variable in skill conditions. Follow them when adding one.

Data, relative to `data/`:

- `skill_data.json`: skill information (duration, targets, effects, conditions, etc.).
- `names.json`: names in Japanese and English, by kind (`characters` by card ID, outfit and Uma; `skills` by skill ID; `tracks` by track ID).
- `course_data.json`: course information (distance, corners, straights, slopes, etc.), keyed by course ID.
- `rails/`: shape of each course's inner rail in the world (base ratio, length, and position and outward lane direction of each keyframe on the horizontal plane), one file per course named after its ID.

Scripts, relative to `scripts/`:

- `make_skill_data.pl`: generates `skill_data.json` from the game's master database (`master.mdb`).
- `make_names.pl`: updates `names.json` from the game's master databases (`master.mdb`), keeping names missing from them.
- `make_skill_level_values.pl`: generates `src/race/skill_scaling/level_values.rs` from the game's master database (`master.mdb`).
- `check_comment_width.pl`: reports comment lines longer than 80 characters in Rust files (`src/` and `tests/` by default), as `file:line`.
