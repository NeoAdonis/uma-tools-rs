# Style guide

The following style guide is intended to be a reference for contributors to this
code base. Contributions are expected to follow these guidelines, which help
maintain consistency and readability in the code and content. Deviations are
accepted when there's a good reason for them, and minor ones don't block a
contribution on their own; they may be pointed out during review and fixed
later.

## Formatting

This code base follows the
[Rust Style Guide](https://doc.rust-lang.org/style-guide/) conventions. Because
it uses the default style, `cargo fmt` can be called as-is to ensure that the
code format is in good shape.

Important conventions:

- 4 space indentation, no tabs.
- Maximum line width of 100 characters, including indentation.
- At most one blank line between items and statements.
- Trailing commas in comma-separated lists.
- No trailing whitespace on the end of any line.
- Each attribute on its own line, indented to the level of the item.

Comments should also follow the guide recommendations. Many formatters,
including non-nightly versions of `rustfmt`, skip formatting comments, so take
special care when writing them. In short:

- Prefer line comments (`//`, `///`, `//!`) to block comments (`/* ... */`,
  `/** ... */`, `/*! ... */`).
- A single space must be placed after the opening and before the closing sigils.
- Prefer to put a comment on its own line. If put after code, it must be
  separated by a single space.
- Comments should be complete sentences; they should start with a capital letter
  and end with a period.
- Comment lines are at most 80 characters long (sigils included, indentation
  excluded), and never go beyond the 100-character line limit.

Additionally, the following conventions should be followed for doc comments:

- Doc comments (`///`) must be placed before attributes.
- First line should consist of a brief description as its own paragraph. One
  line if possible; two lines at most.
- Separate each paragraph by a single blank line.
- Only use inner doc comments (`//!`) for module-level or crate-level
  documentation.

Refer to the official [Rust Style Guide](https://doc.rust-lang.org/style-guide/)
for more details.

## Linting

Beyond formatting, code should be checked with
[Clippy](https://doc.rust-lang.org/clippy/), the official Rust linter, which
catches common mistakes and non-idiomatic code. The code base uses the default
set of lints, so `cargo clippy` can be called as-is. When a lint doesn't apply
to a specific case, it can be silenced on the affected item with an
`#[allow(clippy::...)]` attribute.

## Markdown

Markdown documents follow the default
[markdownlint](https://github.com/DavidAnson/markdownlint) rules, configured in
`.markdownlint-cli2.jsonc`, so `markdownlint-cli2` can be called as-is to check
them. Its `--fix` flag fixes most issues, but not line length:

- Lines are at most 80 characters long. Paragraphs and list items are wrapped
  by hand, with list continuations indented to the item's text.
- Links stay on a single line, so break the line before a link rather than
  inside it.
- A line may only go beyond the limit when nothing past it can be wrapped, such
  as a long link.

## API design

The public API follows the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/). The crate
isn't published and makes no compatibility guarantees yet, so the guidelines
about stability and publishing apply loosely; the ones about the shape of the
API still apply. The rules below are the ones most relevant to this code base.
Check them, and the full guidelines when in doubt, whenever a change:

- Adds or changes a `pub` item, or a re-export in `lib.rs`.
- Adds a dependency whose types could appear in a public signature.
- Adds a type that is read from data files or user input.
- Adds an error type.

Rules:

- **Public surface:** The public API consists of the entry points and their
  results. Everything else is `pub(crate)` or private. `lib.rs` re-exports
  public types explicitly, never with globs.
- **No dependency types in signatures**
  ([C-STABLE](https://rust-lang.github.io/api-guidelines/necessities.html#c-stable)):
  Public signatures don't expose types or traits from other crates, so
  dependencies can be replaced without breaking the API. For example, the random
  seed is a plain `u64` rather than a `rand` type.
- **Validate at the boundary**
  ([C-VALIDATE](https://rust-lang.github.io/api-guidelines/dependability.html#c-validate)):
  Invalid input is rejected when it's parsed or deserialized, with an error.
  Internal invariants are checked with `debug_assert!`.
- **Types over primitives**
  ([C-NEWTYPE](https://rust-lang.github.io/api-guidelines/type-safety.html#c-newtype),
  [C-CUSTOM-TYPE](https://rust-lang.github.io/api-guidelines/type-safety.html#c-custom-type)):
  Domain values with a unit or a restricted range use newtypes (e.g.
  `CourseWidths`), and arguments that select a behavior use enums rather than
  `bool`s, especially when several would be adjacent.
- **Naming**
  ([C-CONV](https://rust-lang.github.io/api-guidelines/naming.html#c-conv),
  [C-GETTER](https://rust-lang.github.io/api-guidelines/naming.html#c-getter),
  [C-CTOR](https://rust-lang.github.io/api-guidelines/predictability.html#c-ctor)):
  Conversions use the `as_`, `to_` and `into_` prefixes according to their cost
  and ownership, getters have no `get_` prefix, and constructors are named `new`
  or `from_*`.
- **Errors**
  ([C-GOOD-ERR](https://rust-lang.github.io/api-guidelines/interoperability.html#c-good-err)):
  Public error types implement `std::error::Error` and `Display`, and their
  messages say what was rejected.
- **Documentation**
  ([C-FAILURE](https://rust-lang.github.io/api-guidelines/documentation.html#c-failure)):
  Public functions that can fail or panic document it in `# Errors` and
  `# Panics` sections.
- **Examples**
  ([C-EXAMPLE](https://rust-lang.github.io/api-guidelines/documentation.html#c-example)):
  Public items have an example, within reason: a link to another item's example
  may be enough. Examples show why someone would use the item, rather than how
  to call it.

## Testing

Tests check behavior, not lines: a test is worth keeping when it asserts
something no other test asserts. Coverage tools show which code runs, not which
values are checked, so they can only point at candidates.

### What to test

- **New assertions only:** Before adding a test, check that no other test
  already asserts the same thing. Repeating an assertion at a different level
  counts as new: a solver or integration test may check that a unit is wired in,
  with one case past each threshold, leaving the boundaries to the unit tests.
- **Broad and focused tests:** Data-driven tests (e.g. every skill in the data)
  carry the bulk. Focused tests cover what they can't check: exact values,
  boundaries, formula terms, formatting and errors.
- **Every test can fail:** A test whose assertions hold whatever the code does
  is removed. Compile-time checks and `#[should_panic]` tests count.
- **Game constants:** Where possible, test a constant through a behavior it
  visibly changes. Otherwise, when a table is the specification, one case per
  shared arm is enough to guard against typos.
- **What not to test:** Mappings between our own types, what the compiler
  already guarantees, behavior the game never produces, and errors from
  libraries that `?` merely forwards. Code that swallows a library error does so
  explicitly, and gets a test.
- **Invalid input:** Test what our code decides is invalid (validation, custom
  deserializers, fallbacks), with cases on both sides of each boundary. Assert
  the reason in the error message (the part our code writes), so a case can't
  pass by failing for another reason, but not text added by libraries, such as
  serde's location.
- **Data assumptions:** Assumptions about data files are checked when loading
  them, so unsupported data is rejected with an error rather than by a
  `debug_assert!`. Each data file has a single compatibility test (e.g.
  `test_skill_data_is_supported`).
- **No mocks:** Tests use real types and data, building whatever they need.

### Structure

- One test per behavior, named after it
  (`test_start_dash_caps_speed_at_its_end`).
- From about three cases with the same call shape, use a table of
  `(inputs, expected)` tuples, a loop, and a message naming the inputs
  (`assert_eq!(actual, expected, "{input:?}")`). Fewer cases use plain
  assertions. If a table ends up with unrelated rows, split it into tests named
  after each rule.
- Sibling one-line formulas may share a test.
- Use `assert_eq!` for exact values, such as constants, and `assert_close!` for
  computed floats.

### Data in tests

- Tests may rely on data that doesn't change once released, such as course
  geometry or names.
- Data the game revises, such as skill values, is only read through helpers in
  `test_utils` (e.g. `data_skill`), so a data update breaks tests in one place.
  When a rule depends on such data, build the data in the test instead.

### Coverage

Full coverage isn't a goal. Code can stay uncovered when a bug there would be
harmless or obvious, or when testing it would cost more than the code itself.
For example:

- Display-only output.
- Arms that differ from a tested one only by a constant.
- Arms the compiler forces but the game never reaches.
- Defensive paths, such as `debug_assert!` failures.

Doc examples don't count as tests: coverage and mutation testing don't run them,
so unit tests also cover what they show, preferably from a different angle or
with different data. A test that ends up close to a doc example is fine.

When removing tests, compare coverage before and after (see
[CONTRIBUTING.md](CONTRIBUTING.md)). Any newly uncovered line needs a reason,
either one of the cases above or one stated in the commit message.

## Commit messages

Commit messages follow the
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
specification.

```text
<type>[(optional scope)]: <description>

[optional body]
```

Messages follow the 50/72 rule: the subject line is at most 50 characters long,
and body lines are wrapped at 72 characters. If the subject can't fit all the
relevant details, move them to the body.

### Types

- `feat`: a new feature or behavior.
- `fix`: a bug fix.
- `refactor`: a code change that neither fixes a bug nor adds a feature.
- `style`: formatting changes that don't affect the meaning of the code.
- `test`: new or updated tests.
- `docs`: documentation changes only, including doc comments.
- `build`: changes to build systems, dependencies or packaging.
- `chore`: other maintenance tasks, such as configuration or example files.

Mixing several types in a single commit should be avoided whenever possible, so
that commits stay focused. When it can't be avoided, the type of the main change
is used.

### Scopes

A scope names the part of the code base that a commit changes. It's either a
module or a commonly used system:

- Top-level modules (`condition`, `race`, `uma`, etc.).
- Well-known submodules: `runner` (including `unready_runner`), `solver`, `hp`,
  `popularity`, `transitions`, `evaluator` and `parser`. Other race mechanics
  use `race`.
- Others: `data` (data files), `deps` (dependencies) and `test` (test helpers
  and infrastructure).

A scope should be added whenever possible, as long as the change clearly belongs
to a single scope. Small adaptations in other modules are fine. Otherwise, leave
the scope out.

## Terminology

The conventions below apply to identifiers, comments, documentation and any
other text in the code base.

The code base is written in American English (e.g. "color" instead of "colour").
Official terms keep their own spelling, even when it differs.

### Official terms

Terms with an official English localization use that localization, rather than
older fan translations, romanized Japanese or English terms from the Japanese
version of the game. For example:

- "Umamusume" as a single word, not "Uma Musume" as separate words.
- Mechanics: "rushed", not "kakari" or "panicking"; "dueling", not "compete
  fight".
- Career modes: "Unity Cup", not "Aoharu Hai"; "Trackblazer", not "Make A New
  Track" or "MANT".
- Skills: "Runaway", not "Great Escape"; "564 Escapades", not "Adventure of
  564".

In prose, mechanics are written in lowercase ("rushed", "last leg"), while
proper names, such as career modes and skills, are capitalized.

### Unofficial terms

Terms without an official English localization use original Japanese name or the
translation most common in the community, never the romanized Japanese name. For
example, 全開スパート can also be referred to as "all-out spurt", but not as "zenkai
spurt".

### Aliases

The documentation that first introduces a concept, such as the top of the module
that implements it, lists its common aliases, including the original Japanese
name. This is the only place where romanized names and other alternative names
appear.

### Exceptions

- Race styles in JSON files are serialized as "Oonige", "Nige", "Senkou", "Sasi"
  and "Oikomi" to stay compatible with other tools. Rust code uses the English
  names: "Runaway", "Front", "Pace", "Late" and "End".
- Variable names in skill conditions, and the Rust identifiers that mirror them,
  keep their original spelling (for example,
  `running_style_temptation_opponent_count_nige` and `RunAtFullSpeedRandom`).

### Preferred terms

Some concepts have several common names. To avoid confusion, the code base uses
the following ones:

- **Last leg:** The final phase of the race. It's not called the "last spurt",
  because that name refers to the runner's last spurt mode.
- **All-out spurt:** The 全開スパート mechanic. It's not called:
  - "Zenkai spurt", because it's a romanized name.
  - "Full throttle", because it could be confused with the skill named "Full
    Throttle".
  - "Full speed" or "full-speed spurt", even though the game's own code seems to
    use that name (see the `run_at_full_speed_random` condition variable),
    because it could be confused with a runner reaching her target or maximum
    speed.
