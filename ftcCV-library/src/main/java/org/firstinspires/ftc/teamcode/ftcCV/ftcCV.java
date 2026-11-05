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
