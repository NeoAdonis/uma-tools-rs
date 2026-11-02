# Umamusume race simulator in Rust

A Rust library for simulating Umamusume: Pretty Derby races at scale.

The simulation is based on the analysis of the game's race mechanics made by KuromiAK, as explained in [this write-up](https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0).

Read [AGENTS.md](./AGENTS.md) for general guidelines for this repository.

## Final objectives

- Given a specific Umamusume, her initial stats and an initial set of skills, simulate multiple races adding different combinations of skills to evaluate their impact on performance.
- Given two or more Umamusume with their initial stats and skills, simulate multiple races to evaluate statistically which Umamusume performs better under different conditions.
- Collect and analyze data from the simulations to determine which skills or combinations of skills yield the best results for specific race types and distances.
- Provide insights and recommendations for optimal skill selection based on race outcomes.
- Aim for correctness and statistical significance in the results.
- Because of the large number of skill combinations and races needed for statistically significant results, optimize the simulations for speed and efficiency without sacrificing simplicity.

## Getting started

The project requires a [Rust toolchain](https://www.rust-lang.org/tools/install) with support for the 2024 edition. To build and run the demo app, run the following from the repository root:

```sh
cargo run --release
```

The demo simulates a single race and prints a summary of it. By default, it reads its input from `config.json` (simulation settings), `uma.json` (the Umamusume, as an array exported by UmaExtractor, of which the first `gates` race, with the mood set by `defaultMood`) and `parameters.json` (course and race conditions). Use `--config`, `--uma` and `--parameters` to read other files instead; for example, `cargo run --release -- --uma my_uma.json`.

To simulate the same race many times instead, run `cargo run --release --bin multi_race`. It reads the same files and accepts the same options, plus `--races` for the number of races (1000 by default). Race N uses the configured seed plus N (or a random base seed if none is set), and the app prints the simulation speed and the finish time statistics of every runner. To profile it, build with `--profile profiling`, which keeps debug info in an optimized build.

> [!NOTE]
> The demo is still very limited compared to the objectives above. For now, it serves mostly as a way to try out the simulation.

## License

Licensed under the [MIT license](LICENSE).
