//! Stepper motor driver for 28BYJ-48 with ULN2003
//!
//! Supports full-step and half-step modes with async operation.

use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Level, Output};

/// Motor rotation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Direction {
    Clockwise,
    CounterClockwise,
}

/// Stepping mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum StepMode {
    /// 4-step sequence, 2048 steps per revolution
    Full,
    /// 8-step sequence, 4096 steps per revolution (smoother)
    Half,
}

impl StepMode {
    /// Steps required for one full revolution of the output shaft
    pub const fn steps_per_revolution(self) -> u32 {
        match self {
            StepMode::Full => 2048,
            StepMode::Half => 4096,
        }
    }
}

/// Full-step sequence (4 steps)
/// Each row: [IN1, IN2, IN3, IN4]
const FULL_STEP_SEQUENCE: [[bool; 4]; 4] = [
    [true, true, false, false],
    [false, true, true, false],
    [false, false, true, true],
    [true, false, false, true],
];

/// Half-step sequence (8 steps) - smoother operation
/// Each row: [IN1, IN2, IN3, IN4]
const HALF_STEP_SEQUENCE: [[bool; 4]; 8] = [
    [true, false, false, false],
    [true, true, false, false],
    [false, true, false, false],
    [false, true, true, false],
    [false, false, true, false],
    [false, false, true, true],
    [false, false, false, true],
    [true, false, false, true],
];

/// 28BYJ-48 stepper motor driver
pub struct Stepper<'d> {
    in1: Output<'d>,
    in2: Output<'d>,
    in3: Output<'d>,
    in4: Output<'d>,
    step_mode: StepMode,
    current_step: usize,
    step_delay_us: u64,
}

impl<'d> Stepper<'d> {
    /// Create a new stepper motor driver
    ///
    /// Default: Half-step mode at 10 RPM
    pub fn new(in1: Output<'d>, in2: Output<'d>, in3: Output<'d>, in4: Output<'d>) -> Self {
        let mut stepper = Self {
            in1,
            in2,
            in3,
            in4,
            step_mode: StepMode::Half,
            current_step: 0,
            step_delay_us: 0,
        };
        stepper.set_rpm(10.0);
        stepper
    }

    /// Set the stepping mode
    pub fn set_mode(&mut self, mode: StepMode) {
        self.step_mode = mode;
        let current_rpm = self.get_rpm();
        self.set_rpm(current_rpm);
    }

    /// Get current stepping mode
    pub fn mode(&self) -> StepMode {
        self.step_mode
    }

    /// Set motor speed in RPM (revolutions per minute)
    ///
    /// 28BYJ-48 is rated for max 15 RPM at 5V
    pub fn set_rpm(&mut self, rpm: f32) {
        let rpm = rpm.clamp(0.1, 15.0);
        let steps_per_rev = self.step_mode.steps_per_revolution() as f32;
        let steps_per_second = (rpm * steps_per_rev) / 60.0;
        self.step_delay_us = (1_000_000.0 / steps_per_second) as u64;
    }

    /// Get current speed in RPM
    pub fn get_rpm(&self) -> f32 {
        let steps_per_rev = self.step_mode.steps_per_revolution() as f32;
        let steps_per_second = 1_000_000.0 / self.step_delay_us as f32;
        (steps_per_second * 60.0) / steps_per_rev
    }

    /// Set step delay directly in microseconds (minimum 1000us)
    pub fn set_step_delay_us(&mut self, delay_us: u64) {
        self.step_delay_us = delay_us.max(1000);
    }

    /// Perform a single step in the given direction
    pub async fn step(&mut self, direction: Direction) {
        let sequence_len = match self.step_mode {
            StepMode::Full => FULL_STEP_SEQUENCE.len(),
            StepMode::Half => HALF_STEP_SEQUENCE.len(),
        };

        self.current_step = match direction {
            Direction::Clockwise => (self.current_step + 1) % sequence_len,
            Direction::CounterClockwise => {
                if self.current_step == 0 {
                    sequence_len - 1
                } else {
                    self.current_step - 1
                }
            }
        };

        let pattern = match self.step_mode {
            StepMode::Full => FULL_STEP_SEQUENCE[self.current_step],
            StepMode::Half => HALF_STEP_SEQUENCE[self.current_step],
        };

        self.set_pins(pattern);
        Timer::after(Duration::from_micros(self.step_delay_us)).await;
    }

    /// Perform multiple steps in the given direction
    pub async fn steps(&mut self, count: u32, direction: Direction) {
        for _ in 0..count {
            self.step(direction).await;
        }
    }

    /// Rotate by a specific angle in degrees
    pub async fn rotate(&mut self, degrees: f32, direction: Direction) {
        let steps_per_rev = self.step_mode.steps_per_revolution() as f32;
        let steps = ((degrees.abs() / 360.0) * steps_per_rev) as u32;
        self.steps(steps, direction).await;
    }

    /// Perform one full revolution
    pub async fn revolution(&mut self, direction: Direction) {
        let steps = self.step_mode.steps_per_revolution();
        self.steps(steps, direction).await;
    }

    /// Stop the motor and de-energize all coils
    ///
    /// Important: Call this when motor is idle to prevent overheating
    pub fn stop(&mut self) {
        self.set_pins([false, false, false, false]);
    }

    /// Set GPIO pins according to pattern [IN1, IN2, IN3, IN4]
    fn set_pins(&mut self, pattern: [bool; 4]) {
        self.in1.set_level(Self::bool_to_level(pattern[0]));
        self.in2.set_level(Self::bool_to_level(pattern[1]));
        self.in3.set_level(Self::bool_to_level(pattern[2]));
        self.in4.set_level(Self::bool_to_level(pattern[3]));
    }

    fn bool_to_level(value: bool) -> Level {
        if value { Level::High } else { Level::Low }
    }
}

impl Drop for Stepper<'_> {
    fn drop(&mut self) {
        self.stop();
    }
}
