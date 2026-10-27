# Adding Condition Variables

How to support a new variable in skill conditions. For how the skill activation
system works, see the `trigger` module docs (`src/trigger.rs`).

Variables fall into three kinds, from simplest to most involved:

- **Regular variables** can be evaluated at any moment (e.g. `hp_per`).
- **Random variables** pick where the skill can activate (e.g. `phase_random`).
- **Events** are only true at specific moments, so they can't be evaluated at an
  arbitrary one (e.g. `is_last_straight_onetime`).

Whatever the kind, also:

- Describe the variable in `docs/CONDITIONS.md`, in alphabetical order.
- Add tests for each step below that applies (parsing, evaluation, ranges,
  rolls, triggers).

## Regular Variable

1. Add it to its category enum in `src/condition/variables.rs` (or to a new
   category in `VarType`), and map its name in `VarType::parse`. The `Display`
   of category enums is derived from the variant name in snake case, so the
   variant must mirror the condition name exactly (e.g. `Visiblehorse` for
   `visiblehorse`).
2. If its value is a set whose comparisons test membership (e.g. of a skill
   ID), or the variable is a flag, restrict the operators and values it
   supports in `Comparison::is_supported` (`src/condition/parser.rs`), so that
   anything else is rejected when parsing.
3. If it can't change during a race, add it to `VarType::is_static`, so its
   guards are resolved when the runner is created.
4. If it's known before the race and an unmet comparison on it should prevent a
   skill from adding popularity points, add it to `VarType::is_popularity_guard`.
5. Return its value in `src/condition/evaluator.rs`:
   - In `variable_value`, as a `Value::Set` if comparisons test whether
     their value is in a set (e.g. a skill ID among the activated ones), or a
     `Value::Range` if it's only known up to rounding (e.g. the order rates
     that round to the runner's order).
   - In `variable_value_out_of_gate` too, if it's known before the race.
   - If it's out of scope, make the evaluator panic on it, and list it under
     "Unimplemented Variables" in the module docs.
6. If its value depends on the position (or time, or HP), add its range to
   `position_range` (or `time_range`, `hp_range`) in
   `src/trigger/skill_activation.rs`, so windows can narrow down when it's
   checked. Otherwise, nothing else is needed: it's a guard.

## Random Variable

1. Add it to `RandomPhaseVar` or `RandomSpatialVar` in
   `src/condition/variables.rs`, and map its name in `VarType::parse`. Pure
   probability rolls go in `RandomVar` instead, like `random_lot`. The parser
   already restricts every variable in these categories to `==`.
2. Roll it in `process_random_comp` (`src/trigger/random.rs`): return the start
   of each picked 10m segment (use `random_segment` /
   `random_segment_in_intervals`), or `None` if it can't be rolled on this
   course.
3. Nothing else is needed: `compile_branch` turns the segments into a `Segments`
   trigger trimmed to the rest of the branch, and the runner tests the guards
   every frame while inside them. The evaluator already panics on random
   variables, since they're never kept as guards.
4. If the area can only be known during the race (like
   `run_at_full_speed_random`), return no segments and handle it as an event
   instead (see below): its trigger pushes its segment windows to
   `position_windows` once the area is known.

## Event

Prefer existing triggers when possible: anything that can be expressed as a
range of positions (like random segments) should become a position window,
which needs no new code in the runner.

1. Add the variable as a regular variable (steps 1 and 2 above).
2. Add a `TriggerCondition` variant in `src/trigger/skill_activation.rs`. Make
   `is_event` return true for the variable, and produce the trigger in
   `compile_branch`. If the variable can't be evaluated, don't add it to the
   guards, and make the evaluator panic on it; otherwise, keep it as a guard
   too, like skill activation events.
3. Store the trigger in the scheduler (`index_trigger` in
   `src/trigger/builder.rs`), in a structure that fits how it fires
   (`WindowQueue` for ranges over a value that never decreases, a list for
   events), and remove it in `TriggerScheduler::remove_triggers`
   (`src/trigger/system.rs`).
4. Check it in the runner at the right point of `Runner::check_skills`
   (`src/race/runner/skills.rs`), calling `try_activate_skill`. Detect it during
   the update if needed, store it, and handle it in the next skill check.
5. Decide whether it fires once or retries.
6. List the new trigger in the `system` module docs (`src/trigger/system.rs`),
   and the new event in "Compiling a Branch" in the `skill_activation` module
   docs.
