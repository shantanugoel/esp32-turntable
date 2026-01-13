# Turntable - ESP32-C3 Stepper Motor Controller

## Project Overview

A Rust-based stepper motor controller for ESP32-C3 SuperMini board, controlling a 28BYJ-48 stepper motor via ULN2003 driver. The project enables motor direction and speed control through multiple interfaces: direct control, WiFi, and BLE.

## Hardware Configuration

| Component | Specification |
|-----------|---------------|
| **MCU** | ESP32-C3 SuperMini |
| **Motor** | 28BYJ-48 (15 RPM) |
| **Driver** | ULN2003 |
| **Power** | +5V, GND from ESP32 |

### GPIO Pin Mapping

| ULN2003 Pin | ESP32-C3 GPIO |
|-------------|---------------|
| IN1 | GPIO1 |
| IN2 | GPIO2 |
| IN3 | GPIO3 |
| IN4 | GPIO4 |

## Motor Specifications (28BYJ-48)

- **Step Angle**: 5.625° per step (64 steps per revolution of internal motor)
- **Gear Ratio**: 1:64
- **Steps per Revolution**: 4096 (full-step) / 2048 (half-step) for output shaft
- **Rated Speed**: 15 RPM (at 5V)
- **Operating Voltage**: 5V DC

### Stepping Sequences

**Full-Step (4-step sequence)**:
| Step | IN1 | IN2 | IN3 | IN4 |
|------|-----|-----|-----|-----|
| 1 | 1 | 1 | 0 | 0 |
| 2 | 0 | 1 | 1 | 0 |
| 3 | 0 | 0 | 1 | 1 |
| 4 | 1 | 0 | 0 | 1 |

**Half-Step (8-step sequence)** - smoother operation:
| Step | IN1 | IN2 | IN3 | IN4 |
|------|-----|-----|-----|-----|
| 1 | 1 | 0 | 0 | 0 |
| 2 | 1 | 1 | 0 | 0 |
| 3 | 0 | 1 | 0 | 0 |
| 4 | 0 | 1 | 1 | 0 |
| 5 | 0 | 0 | 1 | 0 |
| 6 | 0 | 0 | 1 | 1 |
| 7 | 0 | 0 | 0 | 1 |
| 8 | 1 | 0 | 0 | 1 |

## Project Structure

```
turntable/
├── src/
│   ├── bin/
│   │   └── main.rs          # Application entry point
│   ├── web/
│   │   └── index.html       # Embedded web UI
│   ├── stepper.rs           # Stepper motor driver
│   ├── wifi.rs              # WiFi connection management
│   ├── http.rs              # HTTP server and API
│   └── lib.rs               # Library root
├── Cargo.toml               # Dependencies and build config
├── rust-toolchain.toml      # Rust toolchain config
├── build.rs                 # Build script
├── .cargo/
│   └── config.toml          # Cargo build settings
└── AGENTS.md                # This file
```

## Technology Stack

- **Language**: Rust (Edition 2024, MSRV 1.88)
- **Target**: `riscv32imc-unknown-none-elf`
- **HAL**: `esp-hal` ~1.0 (with `unstable` feature)
- **Async Runtime**: Embassy (`esp-rtos`, `embassy-executor`, `embassy-time`)
- **Networking**: `esp-radio` (WiFi + BLE coex), `embassy-net`, `smoltcp`
- **BLE**: `trouble-host` with GATT support, `bt-hci`
- **Logging**: `defmt` ecosystem

## Development Phases

### Phase 1: Direct Motor Control (Test)
- [x] Implement stepper motor driver module
- [x] GPIO configuration for IN1-IN4
- [x] Full-step and half-step sequences
- [x] Direction control (CW/CCW)
- [x] Speed control via step delay timing
- [x] Basic test: rotate motor in both directions at various speeds

### Phase 2: WiFi Control
- [x] WiFi station mode connection
- [x] HTTP server with embedded web UI
- [x] REST API endpoints for motor control
- [x] Gear ratio support (1:3 external gear)
- [x] Photography/videography/3D scanning presets

### Phase 3: BLE Control
- [ ] BLE GATT server setup
- [ ] Custom service for motor control
- [ ] Characteristics: direction, speed, position, status
- [ ] Mobile app integration ready

## Coding Conventions

### Rust Style
- Use `#![no_std]` and `#![no_main]` for embedded
- Prefer `defmt` macros over `println!` for logging
- Use Embassy async patterns for non-blocking operations
- Avoid `mem::forget` (denied via clippy)
- Keep stack frames small (denied via clippy)

### Embedded Patterns
- Use `static_cell` for static allocations
- Use `critical-section` for interrupt-safe code
- Prefer async/await over blocking delays
- Handle all `Result` types explicitly

### Package Versioning
- **ALWAYS use latest packages**, including RC/pre-release versions for ESP ecosystem
- Check for updates before major changes
- Document any version-specific workarounds

## Build & Flash

```bash
# Build
cargo build --release

# Build and flash with monitor
cargo run --release

# Just flash (uses espflash via .cargo/config.toml runner)
cargo run --release
```

## Commands Reference

### Motor Control API (Planned)

```rust
// Direction
enum Direction {
    Clockwise,
    CounterClockwise,
}

// Step mode
enum StepMode {
    Full,  // 4-step sequence, 2048 steps/rev
    Half,  // 8-step sequence, 4096 steps/rev
}

// Motor control interface
trait StepperMotor {
    async fn step(&mut self, direction: Direction);
    async fn steps(&mut self, count: u32, direction: Direction);
    async fn set_speed(&mut self, rpm: f32);
    async fn rotate(&mut self, degrees: f32, direction: Direction);
    fn stop(&mut self);
}
```

## Resources

- [esp-hal documentation](https://docs.esp-rs.org/esp-hal/)
- [esp-hal examples](https://github.com/esp-rs/esp-hal/tree/esp-hal-v~1.0/examples)
- [trouble-host BLE examples](https://github.com/embassy-rs/trouble/tree/main/examples/esp32)
- [28BYJ-48 datasheet](https://www.mouser.com/datasheet/2/758/stepd-01-data-sheet-1143075.pdf)
- [Embassy async embedded](https://embassy.dev/)

## Notes

- ULN2003 is a Darlington transistor array - provides current amplification for motor coils
- 28BYJ-48 coils should not be energized continuously at full current to prevent overheating
- Consider implementing motor idle/sleep mode when not moving
- ESP32-C3 GPIOs are 3.3V but ULN2003 inputs work fine with 3.3V logic levels
