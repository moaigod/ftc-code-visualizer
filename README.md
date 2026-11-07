# FTC Code Visualizer

A Rust desktop application for visualizing FTC robot code and testing robot motion on a season field.

## Current status

- Native desktop window built with Rust and egui.
- Uses the supplied BIOBUZZ top-down field screenshot as the field background.
- 144 × 144 inch coordinate space.
- Robot position, heading, and basic motion controls. Heading uses a compass-style convention: 0° points toward the top of the field and increases clockwise.
- Import an FTC Android Studio project and scan TeamCode Java/Kotlin source.
- Detect and list classes annotated with `@TeleOp` or `@Autonomous`.
- Open a read-only source preview for a detected OpMode.
- Controller-driven generic mecanum-style TeleOp movement using a connected gamepad.
- Basic source extraction for telemetry display (not full telemetry execution).
- Extract `telemetry.addData()` and `telemetry.addLine()` entries from the selected OpMode.
- Display live simulator values for recognized position, heading, runtime, and gamepad expressions.

## Field image setup

The app loads the field image from:

```text
assets/biobuzz-field.png
```

Create the `assets` folder in the project root and place the supplied top-down field screenshot there with that exact filename.

## Import an FTC project

Run the app and select **Import FTC project folder**. Choose the root folder of your Android Studio FTC project (the folder containing `TeamCode`, not the `TeamCode` folder itself).

The importer scans `TeamCode/src/main/java` for Java and Kotlin files, detects `@TeleOp` and `@Autonomous` annotations, and lets you inspect their source.

## TeleOp controller

Connect an Xbox-compatible gamepad before launching the app, then press **Start TeleOp**.

- Left stick Y: forward and backward
- Left stick X: strafe
- Right stick X: rotate

This currently drives a generic mecanum-style robot model. It is a controller-based simulator input, not yet an execution of the selected OpMode's code. The telemetry panel extracts simple, single-line `telemetry.addData("caption", expression)` and `telemetry.addLine("message")` calls. It only substitutes a small set of recognized simulator values; it does not execute the expressions or arbitrary FTC SDK code. Autonomous OpModes are detected and can be previewed, but cannot currently be run.

## Run locally

Install the Rust toolchain, then run:

```sh
cargo run
```

## Next development steps

1. Parse FTC hardware mappings and motor/servo declarations.
2. Interpret common gamepad-to-motor logic from TeleOp source.
3. Add autonomous command and sequence interpretation.
4. Add PedroPathing 3 path extraction and rendering.
5. Implement autonomous execution for supported OpMode patterns.
6. Add a separate live telemetry bridge.

## Important limitation

Android FTC OpModes run inside the FTC Android app on the Control Hub or Driver Hub environment. They cannot be executed directly as Windows desktop code. This project imports and interprets supported source patterns to create a desktop simulation; it is not a replacement for the FTC SDK or the Android build.


## BIOBUZZ pollen interaction

The field simulation stages 4 pollen at each of the four Flowers and 4 in each alliance Garden (24 field pollen total). The robot starts with the 4 preloaded pollen specified by the 2026–2027 BIOBUZZ manual.

- Use the front intake near a yellow pollen ball and press **E** or click **Intake pollen**.
- Press **Space** or click **Shoot pollen** to launch one carried pollen.
- The carried count is capped at 4. A launched pollen is animated and returned to the field when its flight ends.
- The Pollen panel displays the robot's current carried count.

Flower pollen is stacked at the edge of each side of the field: bottom (96.5, 1.4), right (142.6, 96.5), top (47.5, 142.6), and left (1.4, 47.5). Each Flower stack contains four overlapping pollen pieces with progressively smaller display sizes. Flower pickups have a 0.5-second cooldown; Garden pickups do not. Garden pollen starts from the two opposite corners and extends inward along the edges. These are visualizer staging coordinates, not a surveyed field CAD model. This requested return-to-field shot behavior is a simulator mechanic; official BIOBUZZ scoring launches pollen into the upward-facing Hive cell.


## STEP CAD field importer

The 3D view can load STEP / STP / P21 CAD assemblies with **Load STEP** in the top bar. The importer tessellates part geometry, expands assembly instances with their placement transforms, applies STEP length units as inches, and uses source surface colours when available. **Use procedural field** restores the built-in fallback geometry. The 2D view and its renderer are unchanged.

The reader is provided by `oxideav-step` (MIT), built with its standard-library-only feature set.
