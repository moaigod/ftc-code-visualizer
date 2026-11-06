# ftcCV hardware declaration library

This is an isolated, optional Java helper library for declaring robot hardware
metadata that the FTC Code Visualizer can read. It is separate from the Rust
visualizer application and has **no FTC SDK dependency**.

It does not initialize motors, access the hardware map, or control the robot.
Your normal FTC code remains responsible for real hardware.

## Add it to an FTC project

Copy `src/main/java/org/firstinspires/ftc/teamcode/ftcCV/ftcCV.java`
into the matching package folder in your FTC TeamCode module, then import it:

```java
import org.firstinspires.ftc.teamcode.ftcCV.ftcCV;
```

## Example

```java
ftcCV.hardware.motor(
    "frontLeft",
    ftcCV.MotorType.DRIVE,
    ftcCV.Direction.REVERSE
);

ftcCV.hardware.motor(
    "intake",
    ftcCV.MotorType.INTAKE
);

ftcCV.hardware.motor(
    "shooter",
    ftcCV.MotorType.FLYWHEEL,
    ftcCV.maxRPM(6000),
    ftcCV.maxAcceleration(3500)
);
```

## Motor types

- `DRIVE`
- `INTAKE`
- `FEEDER`
- `FLYWHEEL`
- `OUTTAKE`
- `OTHER`

Mechanism-specific values are optional. The visualizer matches each declaration
to a `hardwareMap` motor by the exact configured hardware name.

- `maxRPM(value)` sets the motor's configured free-speed limit.
- `maxAcceleration(value)` sets its acceleration limit.
- `Direction.FORWARD` and `Direction.REVERSE` are retained as motor direction metadata.

The drivetrain model uses 312 RPM and 2000 acceleration units as its calibration
reference; the flywheel model uses 6000 RPM and 3500 acceleration units. Values
are scaled relative to those references. Motors without settings retain the
built-in defaults. The current simulator models chassis-level mecanum movement,
so direction metadata is shown in the detected-hardware panel rather than
simulating individual wiring polarity.

## Separation

This folder is intentionally self-contained. The Rust visualizer can have a
separate parser for these declarations; the Java helper does not need to know
about Rust, Rapier, egui, or the simulator's internal implementation.
