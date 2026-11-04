# FTC Code Visualizer

A Rust desktop application for visualizing FTC robot code and testing robot motion on a season field.

## Current status

- Native desktop window built with Rust and egui.
- Uses the supplied BIOBUZZ top-down field screenshot as the field background.
- 144 × 144 inch coordinate space.
- Robot position and heading controls.
- Basic run, pause, and reset motion test.

## Field image setup

The app loads the field image from:

```text
assets/biobuzz-field.png
```

Create the `assets` folder in the project root and place the supplied top-down field screenshot there with that exact filename. The app displays that image directly; it does not redraw or approximate the field elements.

## Run locally

Install the Rust toolchain, then run:

```sh
cargo run
```

## Planned work

1. Calibrate the field image to the official 144 × 144 inch coordinate bounds.
2. Add robot configuration and coordinate transforms.
3. Import FTC Android Studio projects.
4. Parse supported Java/Kotlin OpMode patterns.
5. Add PedroPathing 3 path visualization.
6. Build a deterministic simulation layer and, separately, a live telemetry bridge.

## Important limitation

Arbitrary FTC Android code cannot be executed directly as a Windows desktop simulation. The project will need a supported-code importer/simulation model for offline visualization. Live robot data will require a separate telemetry connection.
