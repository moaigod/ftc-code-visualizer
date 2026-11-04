# FTC Code Visualizer

A Rust desktop application for visualizing FTC robot code and testing robot motion on a season field.

## Current status

The first application scaffold is in place:

- Native desktop window built with Rust and egui.
- 144 × 144 inch field coordinate space.
- Robot position and heading controls.
- Basic run, pause, and reset motion test.
- BIOBUZZ 2026–2027 project label.

The current field drawing is a coordinate-space placeholder, not an accurate BIOBUZZ field model. Official field geometry and scoring locations must be added from the season's official resources before using it for match planning.

## Run locally

Install the Rust toolchain, then run:

```sh
cargo run
```

## Planned work

1. Implement verified BIOBUZZ field geometry and field elements.
2. Add robot configuration and coordinate transforms.
3. Import FTC Android Studio projects.
4. Parse supported Java/Kotlin OpMode patterns.
5. Add PedroPathing 3 path visualization.
6. Build a deterministic simulation layer and, separately, a live telemetry bridge.

## Important limitation

Arbitrary FTC Android code cannot be executed directly as a Windows desktop simulation. The project will need a supported-code importer/simulation model for offline visualization. Live robot data will require a separate telemetry connection.
