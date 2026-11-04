# FTC Code Visualizer

A Rust desktop application for visualizing FTC robot code and testing robot motion on a season field.

## Current status

- Native desktop window built with Rust and egui.
- Uses the supplied BIOBUZZ top-down field screenshot as the field background.
- 144 × 144 inch coordinate space.
- Robot position, heading, and basic motion controls.
- Select an FTC Android Studio project folder and scan its TeamCode Java/Kotlin source.
- Detect and list classes annotated with `@TeleOp` or `@Autonomous`.
- Open a read-only source preview for a detected OpMode.

## Field image setup

The app loads the field image from:

```text
assets/biobuzz-field.png
```

Create the `assets` folder in the project root and place the supplied top-down field screenshot there with that exact filename.

## Import an FTC project

Run the app and select **Import FTC project folder**. Choose the root folder of your Android Studio FTC project (the folder containing `TeamCode`, not the `TeamCode` folder itself).

The current importer scans `TeamCode/src/main/java` for Java and Kotlin files, detects `@TeleOp` and `@Autonomous` annotations, and lets you inspect their source.

This is the first import milestone only. It does not yet interpret motor commands, Road Runner/PedroPathing trajectories, hardware mappings, or execute Android code.

## Run locally

Install the Rust toolchain, then run:

```sh
cargo run
```

## Next development steps

1. Parse FTC hardware mappings and motor/servo declarations.
2. Recognize supported movement commands and build an intermediate robot-action model.
3. Add PedroPathing 3 path extraction and rendering.
4. Simulate supported OpMode behavior on the field.
5. Add a separate live telemetry bridge.

## Important limitation

Android FTC OpModes run inside the FTC Android app on the Control Hub or Driver Hub environment. They cannot be executed directly as Windows desktop code. This project imports and interprets supported source patterns to create a desktop simulation; it is not a replacement for the FTC SDK or the Android build.
