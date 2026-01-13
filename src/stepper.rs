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
    pub const fn steps_per_motor_revolution(self) -> u32 {
        match self {
            StepMode::Full => 2048,
            StepMode::Half => 4096,
        }
    }
}

const FULL_STEP_SEQUENCE: [[bool; 4]; 4] = [
    [true, true, false, false],
    [false, true, true, false],
    [false, false, true, true],
    [true, false, false, true],
];

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

pub struct Stepper<'d> {
    in1: Output<'d>,
    in2: Output<'d>,
    in3: Output<'d>,
    in4: Output<'d>,
    step_mode: StepMode,
    current_step: usize,
    step_delay_us: u64,
    gear_reduction: u32,
}

impl<'d> Stepper<'d> {
    pub fn new(in1: Output<'d>, in2: Output<'d>, in3: Output<'d>, in4: Output<'d>) -> Self {
        let mut stepper = Self {
            in1,
            in2,
            in3,
            in4,
            step_mode: StepMode::Half,
            current_step: 0,
            step_delay_us: 0,
            gear_reduction: 1,
        };
        stepper.set_output_rpm(10.0);
        stepper
    }

    /// Set gear reduction ratio (e.g., 3 means 3:1 reduction, turntable rotates 3x slower than motor)
    /// Motor at 15 RPM with reduction=3 → Turntable at 5 RPM
    pub fn set_gear_reduction(&mut self, reduction: u32) {
        self.gear_reduction = reduction.max(1);
    }

    pub fn steps_per_output_revolution(&self) -> u32 {
        self.step_mode.steps_per_motor_revolution() * self.gear_reduction
    }

    pub fn set_mode(&mut self, mode: StepMode) {
        self.step_mode = mode;
        let current_rpm = self.get_output_rpm();
        self.set_output_rpm(current_rpm);
    }

    pub fn mode(&self) -> StepMode {
        self.step_mode
    }

    /// Set output shaft speed in RPM (accounts for gear reduction)
    /// Max output RPM = 15 / gear_reduction (motor max is 15 RPM)
    pub fn set_output_rpm(&mut self, output_rpm: f32) {
        let motor_rpm = output_rpm * self.gear_reduction as f32;
        let motor_rpm = motor_rpm.clamp(0.1, 15.0);
        let steps_per_rev = self.step_mode.steps_per_motor_revolution() as f32;
        let steps_per_second = (motor_rpm * steps_per_rev) / 60.0;
        self.step_delay_us = (1_000_000.0 / steps_per_second) as u64;
    }

    pub fn get_output_rpm(&self) -> f32 {
        let steps_per_rev = self.step_mode.steps_per_motor_revolution() as f32;
        let steps_per_second = 1_000_000.0 / self.step_delay_us as f32;
        let motor_rpm = (steps_per_second * 60.0) / steps_per_rev;
        motor_rpm / self.gear_reduction as f32
    }

    pub fn max_output_rpm(&self) -> f32 {
        15.0 / self.gear_reduction as f32
    }

    pub fn set_step_delay_us(&mut self, delay_us: u64) {
        self.step_delay_us = delay_us.max(1000);
    }

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

    pub async fn steps(&mut self, count: u32, direction: Direction) {
        for _ in 0..count {
            self.step(direction).await;
        }
    }

    /// Rotate output shaft by specific angle in degrees
    pub async fn rotate(&mut self, degrees: f32, direction: Direction) {
        let steps_per_rev = self.steps_per_output_revolution() as f32;
        let steps = ((degrees.abs() / 360.0) * steps_per_rev) as u32;
        self.steps(steps, direction).await;
    }

    /// Perform one full revolution of output shaft
    pub async fn revolution(&mut self, direction: Direction) {
        let steps = self.steps_per_output_revolution();
        self.steps(steps, direction).await;
    }

    pub fn stop(&mut self) {
        self.set_pins([false, false, false, false]);
    }

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
