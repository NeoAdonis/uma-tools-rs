# Umamusume race simulator in Rust

A Rust library for simulating *Umamusume: Pretty Derby* races at scale.

It simulates whole races frame by frame, from a single runner to a full field,
following the game's mechanics and skill activation, and runs the same race many
times to compare runners and skills statistically. It's made of a race
simulator, a skill activation system and condition parser, course and skill data
generated from the game's own data, importers for umadump and uma.moe, and a
command-line tool.

The project is in an early stage: its API may change at any time, and it isn't
published on crates.io yet.

## Features

### Race simulation

- **Frame-by-frame simulation of whole races**: with the game's formulas for
  speed, acceleration, HP, lane changes, blocking and overtaking.
- **Most race mechanics**: rushing, positioning, spot struggle, secure lead,
  compete before spurt and Stamina Keep, Charge Up, last spurt, all-out spurt
  (aka zenkai spurt), dueling, slopes, stamina and wit limit breaks, and
  popularity.
- **Real course geometry**: movement follows each course's inner rail, so
  running wide through corners or changing lanes costs extra distance as it does
  in the game.
- **Mechanics by game version**: as they ran after each major update, from the
  original release to the 5th Anniversary, to measure what an update changed.
- **Race results as in the game**: finish times, placings and finish margins
  (nose, head, neck, lengths or distance).

### Skills

- **Data for every skill in the game**: effects, durations, cooldowns, targets
  and scaling by level or situation, generated from the game's master database.
- **Condition parser and evaluator**: it uses the same format as the game's
  skill condition strings, supporting over 150 condition variables (see
  [CONDITIONS.md](docs/CONDITIONS.md)).
- **Full skill activation logic**: wit checks, random activation points,
  preconditions, passive skills (e.g. stat boosts), effects on other runners
  (e.g. debuffs) and skills activating other skills.
- **Fast activation checks**: conditions are compiled into triggers, which
  decide when a skill is worth checking, and guards, which decide whether it
  activates, so most skills cost nothing on most frames.

### Statistical analysis

- **Reproducible races**: every source of randomness (per runner, mechanic and
  skill) draws from its own stream derived from the race seed. Two races with
  the same seed that only differ in one skill stay identical until that skill
  makes a difference, which reduces the variance when comparing them.
- **Controlled conditions**: wit and rush checks can be forced to always pass or
  fail, and gates and moods can be fixed or randomized.

### Data and import

- Course data (distance, corners, straights, slopes, rails) and race conditions
  (track condition, weather, season, time of day, grade).
- Import of trained Umas exported by
  [**umadump**](https://github.com/Werseter/umadump), and of whole race setups
  (course, conditions, runners, teams and gates) shared as
  [**uma.moe**](https://uma.moe/) build codes.
- Character, skill and track names in Japanese and English.

## Further goals

- **Skill combination search**: given an Uma and her current skills, evaluate
  how much each combination of candidate skills improves her performance,
  building on the paired races the library already supports.
- **Comparisons across conditions**: compare two or more Umas over many courses
  and race conditions, rather than a single race setup.
- **Aggregate analysis**: find which skills or combinations of skills perform
  best for each race type and distance.
- **Recommendations**: suggest which skills to pick based on those results.
- **HP calculator**: check how often an Uma has enough HP for a full last spurt
  on a given course and race conditions, and how much stamina (or which recovery
  skills) she needs to always have it.
- **Statistical significance**: report confidence intervals or significance
  tests, and how many races a comparison needs.
- **Machine-readable output**: export the demo's results as JSON, including
  per-race details.
- **WebAssembly support**: build the library for WebAssembly, so that web tools
  can run simulations in the browser without a server.
- **Correctness and speed**: keep closing the gaps with the game's mechanics,
  and keep simulations fast enough for the many skill combinations and races
  above, without sacrificing simplicity.

## Getting started

The project requires a [Rust toolchain](https://www.rust-lang.org/tools/install)
with support for the 2024 edition. To build and run the demo app, run the
following from the repository root:

```sh
cargo run --release
```

The demo simulates the same race many times and summarizes how every runner
performed. By default, it reads its input from `config.json` (simulation
settings), `uma.json` (an array of Umamusume exported by umadump, of which the
first `gates` race, with the mood set by `defaultMood`) and `parameters.json`
(course and race conditions). Use `--config`, `--uma` and `--parameters` to read
other files instead; for example, `cargo run --release -- --uma my_uma.json`.
Alternatively, use `--build` to race the setup of an uma.moe build code. Use
`--focus` to pick the runner (by field index, 0 by default) whose skill
activations are reported.

`--races` sets the number of races (1000 by default), and `--threads` the number
of threads to split them across (one per logical CPU by default). Race N uses
the configured seed plus N (or a random base seed if none is set), so any race
can be reproduced on its own with that seed. To profile the app, build with
`--profile profiling`, which keeps debug info in an optimized build, and pass
`--threads 1`.

The demo only prints its results as text for now. Exporting them in a
machine-readable format, such as JSON, is planned.

To use the library in your own code, add it as a git dependency. Run
`cargo doc --open` to browse its API docs, whose examples show how to set up and
run races, compare skills over paired seeds, import Umas and more.

## Contributing

This is a personal project, but issues and pull requests are welcome. Before
opening one, read:

- [CONTRIBUTING.md](CONTRIBUTING.md): what to include in an issue, and what a
  pull request should go through before it's submitted.
- [AGENTS.md](AGENTS.md): the project overview for both humans and coding
  agents. It explains the design philosophy, code structure and commit rules.
- [STYLE.md](STYLE.md): conventions for formatting, linting, API design, commit
  messages and terminology.

## License

The code here is licensed under the [MIT license](LICENSE). Forks and reuse are
welcome! Credit beyond the license's copyright notice isn't required, but a link
back to this repository is always appreciated.

This is an independent fan project, not affiliated with or endorsed by Cygames,
Inc. The game's names and any data derived from its files belong to their
respective owners, and aren't covered by the MIT license.

## Acknowledgments

I'd like to thank these projects and websites, which this one builds on or
learned from. Take a look at them!

- [pecan's uma-tools](https://github.com/alpha123/uma-tools): the original
  *Umalator*.
- [mackoz's uma-tools](https://github.com/mackoz/uma-tools): a fork of pecan's
  uma-tools with several simulation accuracy fixes.
- [umadump](https://github.com/Werseter/umadump): an Umamusume veteran list data
  extractor.
- [uma.moe](https://uma.moe/): an Umamusume resource hub and collection of tools
  for race analysis.
- [KuromiAK's Race Mechanics
  write-up](https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit):
  an analysis of the game's race mechanics, whose formulas this simulation is
  based on.
- [Hakuraku's Research Notes](https://hakuraku.moe/notes): research notes on the
  game's mechanics, which helped clear up some of their details.
- [GameTora's Umamusume site](https://gametora.com/umamusume): a reference site
  for skills and conditions.
