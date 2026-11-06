package org.firstinspires.ftc.teamcode.ftcCV;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Optional hardware metadata for the FTC Code Visualizer.
 *
 * This class deliberately has no FTC SDK dependency. It records declarations
 * for the visualizer to read; it does not initialize or control real hardware.
 */
public final class ftcCV {
    private ftcCV() {}

    public static final Hardware hardware = new Hardware();

    public enum MotorType {
        DRIVE, INTAKE, FEEDER, FLYWHEEL, OUTTAKE, OTHER
    }

    public enum PinpointPodType {
        GOBILDA_4_BAR, GOBILDA_SWINGARM, CUSTOM
    }

    public enum EncoderDirection {
        FORWARD, REVERSED
    }

    public static final class PinpointConfig {
        public final String name;
        public final PinpointPodType podType;
        public final double xOffsetMm;
        public final double yOffsetMm;
        public final EncoderDirection xDirection;
        public final EncoderDirection yDirection;
        public final Double ticksPerMm;

        private PinpointConfig(String name, PinpointPodType podType,
                               double xOffsetMm, double yOffsetMm,
                               EncoderDirection xDirection, EncoderDirection yDirection,
                               Double ticksPerMm) {
            this.name = name;
            this.podType = podType;
            this.xOffsetMm = xOffsetMm;
            this.yOffsetMm = yOffsetMm;
            this.xDirection = xDirection;
            this.yDirection = yDirection;
            this.ticksPerMm = ticksPerMm;
        }
    }

    public interface MotorOption {}

    public enum Direction implements MotorOption {
        FORWARD, REVERSE
    }

    public static final class MaxRPM implements MotorOption {
        public final double value;
        public MaxRPM(double value) {
            if (!Double.isFinite(value) || value <= 0) {
                throw new IllegalArgumentException("MaxRPM must be positive");
            }
            this.value = value;
        }
    }

    public static final class MaxAcceleration implements MotorOption {
        public final double value;
        public MaxAcceleration(double value) {
            if (!Double.isFinite(value) || value <= 0) {
                throw new IllegalArgumentException("MaxAcceleration must be positive");
            }
            this.value = value;
        }
    }

    public static MaxRPM maxRPM(double value) {
        return new MaxRPM(value);
    }

    public static MaxAcceleration maxAcceleration(double value) {
        return new MaxAcceleration(value);
    }

    public static final class MotorConfig {
        public final String name;
        public final MotorType type;
        public final Direction direction;
        public final Double maxRPM;
        public final Double maxAcceleration;

        private MotorConfig(String name, MotorType type, Direction direction,
                            Double maxRPM, Double maxAcceleration) {
            this.name = name;
            this.type = type;
            this.direction = direction;
            this.maxRPM = maxRPM;
            this.maxAcceleration = maxAcceleration;
        }
    }

    public static final class Hardware {
        private final List<MotorConfig> motors = new ArrayList<>();
        private PinpointConfig pinpoint;

        /**
         * Declares a goBILDA Pinpoint odometry computer. Offsets are in mm
         * relative to the robot center; encoder directions are independent.
         * For CUSTOM pods, provide ticksPerMm; for presets it may be null.
         */
        public synchronized PinpointConfig pinpoint(
                String name, PinpointPodType podType,
                double xOffsetMm, double yOffsetMm,
                EncoderDirection xDirection, EncoderDirection yDirection) {
            return pinpoint(name, podType, xOffsetMm, yOffsetMm,
                    xDirection, yDirection, null);
        }

        public synchronized PinpointConfig pinpoint(
                String name, PinpointPodType podType,
                double xOffsetMm, double yOffsetMm,
                EncoderDirection xDirection, EncoderDirection yDirection,
                Double ticksPerMm) {
            if (name == null || name.trim().isEmpty() || podType == null
                    || xDirection == null || yDirection == null) {
                throw new IllegalArgumentException("Pinpoint name, pod type, and directions are required");
            }
            if (!Double.isFinite(xOffsetMm) || !Double.isFinite(yOffsetMm)
                    || (ticksPerMm != null && (!Double.isFinite(ticksPerMm) || ticksPerMm <= 0))) {
                throw new IllegalArgumentException("Pinpoint offsets must be finite and ticks/mm must be positive");
            }
            pinpoint = new PinpointConfig(name.trim(), podType, xOffsetMm, yOffsetMm,
                    xDirection, yDirection, ticksPerMm);
            return pinpoint;
        }

        public synchronized PinpointConfig getPinpoint() {
            return pinpoint;
        }

        /**
         * Declares a motor for simulator metadata. Re-declaring a name replaces
         * its previous declaration, so configuration files can be safely reloaded.
         */
        public synchronized MotorConfig motor(
                String name, MotorType type, MotorOption... options) {
            if (name == null || name.trim().isEmpty()) {
                throw new IllegalArgumentException("Motor name cannot be empty");
            }
            if (type == null) {
                throw new IllegalArgumentException("Motor type cannot be null");
            }

            Direction direction = Direction.FORWARD;
            Double rpm = null;
            Double acceleration = null;

            if (options != null) {
                for (MotorOption option : options) {
                    if (option instanceof Direction) {
                        direction = (Direction) option;
                    } else if (option instanceof MaxRPM) {
                        rpm = ((MaxRPM) option).value;
                    } else if (option instanceof MaxAcceleration) {
                        acceleration = ((MaxAcceleration) option).value;
                    } else if (option != null) {
                        throw new IllegalArgumentException(
                                "Unsupported motor option: " + option.getClass().getName());
                    }
                }
            }

            MotorConfig config = new MotorConfig(
                    name.trim(), type, direction, rpm, acceleration);
            for (int i = 0; i < motors.size(); i++) {
                if (motors.get(i).name.equals(config.name)) {
                    motors.set(i, config);
                    return config;
                }
            }
            motors.add(config);
            return config;
        }

        public synchronized List<MotorConfig> getMotors() {
            return Collections.unmodifiableList(new ArrayList<>(motors));
        }
    }
}
