# Style guide

The following style guide is intended to be a reference for contributors to this code base. It is not meant to be a strict set of rules, but rather a set of guidelines to help maintain consistency and readability in the code.

## Formatting

This code base follows the [Rust Style Guide](https://doc.rust-lang.org/style-guide/) conventions. Because it uses the default style, `cargo fmt` can be called as-is to ensure that the code format is in good shape.

Important conventions:

- 4 space indentation, no tabs.
- Maximum line width of 100 characters, including indentation.
- At most one blank line between items and statements.
- Trailing commas in comma-separated lists.
- No trailing whitespace on the end of any line.
- Each attribute on its own line, indented to the level of the item.

Comments should also follow the guide recommendations. Many formatters, including non-nightly versions of `rustfmt`, skip formatting comments, so take special care when writing them. In short:

- Prefer line comments (`//`, `///`, `//!`) to block comments (`/* ... */`, `/** ... */`, `/*! ... */`).
- A single space must be placed after the opening and before the closing sigils.
- Prefer to put a comment on its own line. If put after code, it must be separated by a single space.
- Comments should be complete sentences; they should start with a capital letter and end with a period.
- Limit comment lines to 80 characters (sigils included, indentation excluded), and never beyond the 100-character line limit.

Additionally, the following conventions should be followed for doc comments:

- Doc comments (`///`) must be placed before attributes.
- First line should consist of a brief description as its own paragraph. One line if possible; two lines at most.
- Separate each paragraph by a single blank line.
- Only use inner doc comments (`//!`) for module-level or crate-level documentation.

Refer to the official [Rust Style Guide](https://doc.rust-lang.org/style-guide/) for more details.

## Linting

Beyond formatting, code should be checked with [Clippy](https://doc.rust-lang.org/clippy/), the official Rust linter, which catches common mistakes and non-idiomatic code. The code base uses the default set of lints, so `cargo clippy` can be called as-is. When a lint doesn't apply to a specific case, it can be silenced on the affected item with an `#[allow(clippy::...)]` attribute.

## Commit messages

Commit messages follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification.

```text
<type>[(optional scope)]: <description>

[optional body]
```

Messages follow the 50/72 rule: the subject line is at most 50 characters long, and body lines are wrapped at 72 characters. If the subject can't fit all the relevant details, move them to the body.

### Types

- `feat`: a new feature or behavior.
- `fix`: a bug fix.
- `refactor`: a code change that neither fixes a bug nor adds a feature.
- `style`: formatting changes that don't affect the meaning of the code.
- `test`: new or updated tests.
- `docs`: documentation changes only, including doc comments.
- `build`: changes to build systems, dependencies or packaging.
- `chore`: other maintenance tasks, such as configuration or example files.

Commits that mix several types use the type of their main change.

### Scopes

A scope names the part of the code base that a commit changes. It's either a module or a commonly used system:

- Top-level modules (`condition`, `race`, `uma`, etc.).
- Well-known submodules: `runner` (including `unready_runner`), `solver`, `hp`, `popularity`, `transitions`, `evaluator` and `parser`. Other race mechanics use `race`.
- Others: `data` (data files), `deps` (dependencies) and `test` (test helpers and infrastructure).

A scope should be added whenever possible, as long as the change clearly belongs to a single scope. Small adaptations in other modules are fine. Otherwise, leave the scope out.

## Terminology

The conventions below apply to identifiers, comments, documentation and any other text in the code base.

The code base is written in American English (e.g. "color" instead of "colour"). Official terms keep their own spelling, even when it differs.

### Official terms

Terms with an official English localization use that localization, rather than older fan translations, romanized Japanese or English terms from the Japanese version of the game. For example:

- "Umamusume" as a single word, not "Uma Musume" as separate words.
- Mechanics: "rushed", not "kakari" or "panicking"; "dueling", not "compete fight".
- Career modes: "Unity Cup", not "Aoharu Hai"; "Trackblazer", not "Make A New Track" or "MANT".
- Skills: "Runaway", not "Great Escape"; "564 Escapades", not "Adventure of 564".

In prose, mechanics are written in lowercase ("rushed", "last leg"), while proper names, such as career modes and skills, are capitalized.

### Unofficial terms

Terms without an official English localization use original Japanese name or the translation most common in the community, never the romanized Japanese name. For example, 全開スパート can also be referred to as "all-out spurt", but not as "zenkai spurt".

### Aliases

The documentation that first introduces a concept, such as the top of the module that implements it, lists its common aliases, including the original Japanese name. This is the only place where romanized names and other alternative names appear.

### Exceptions

- Race styles in JSON files are serialized as "Oonige", "Nige", "Senkou", "Sasi" and "Oikomi" to stay compatible with other tools. Rust code uses the English names: "Runaway", "Front", "Pace", "Late" and "End".
- Variable names in skill conditions, and the Rust identifiers that mirror them, keep their original spelling (for example, `running_style_temptation_opponent_count_nige` and `RunAtFullSpeedRandom`).

### Preferred terms

Some concepts have several common names. To avoid confusion, the code base uses the following ones:

- **Last leg:** The final phase of the race. It's not called the "last spurt", because that name refers to the runner's last spurt mode.
- **All-out spurt:** The 全開スパート mechanic. It's not called:
  - "Zenkai spurt", because it's a romanized name.
  - "Full throttle", because it could be confused with the skill named "Full Throttle".
  - "Full speed" or "full-speed spurt", even though the game's own code seems to use that name (see the `run_at_full_speed_random` condition variable), because it could be confused with a runner reaching her target or maximum speed.
