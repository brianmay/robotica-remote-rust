# Robotica Remote Rust

Copied and adapted from https://github.com/ivmarkov/rust-esp32-std-hello.


Highlights:

* Pure Rust implementation.

Todo:

* Revise error handling. Anything unexpected will panic with limited debug
  information.
* Only one button controller implemented, and is specific to Robotica lights.
* Button config is hardcoded.
* Fix dodgy code to get topic from mqtt message. See https://github.com/ivmarkov/rust-esp32-std-demo/issues/64

Assumptions:

* Use with this board: http://www.openhardwareconf.org/wiki/SwagBadge2021
* gpio16: 1st button, pulled high, action low.
* gpio16: 2nd button, pulled high, action low.
* 2 ssd1306 compatable displays on i2c, scl gpio4, sda gpio5, addr 0x3C and 0x3D.
* slider controls not yet used.


## Design

```mermaid
graph TD
    Main((Main))
    Mqtt((Mqtt))
    Mqtt --> |event| Main
    Main --> |cmd| Mqtt
    Button1((Button1)) --> |event| Main
    Button2((Button2)) --> |event| Main
    Main --> |status| Display((Display))
    Display --> Display1
    Display --> Display2
    button1 --> Button1
    button2 --> Button2
    mqtt --> |event| Mqtt
    Mqtt --> |cmd| mqtt
```

### Main Process

```mermaid
graph TD
    Main((Main))
    LightController((LC))
    SwitchController((SC))
    LightController --> |status| Main
    LightController --> |cmd| Main
    Main --> |event| LightController
    SwitchController --> |status| Main
    SwitchController --> |cmd| Main
    Main --> |event| SwitchController
```
## Build

- Install the [Rust Espressif compiler toolchain and the Espressif LLVM Clang toolchain](https://github.com/esp-rs/rust-build)
  - This is necessary, because support for the Xtensa architecture (ESP32 / ESP32-S2 / ESP32-S3) is not upstreamed in LLVM yet
- Switch to the `esp` toolchain from the pre-built binaries: `rustup default esp`
  - (You can also skip this step and switch to the `esp` toolchain *for the demo crate only* by executing `rustup override set esp` inside the `rust-esp32-std-demo` directory once you have cloned the demo as per below)
  - **NOTE** For ESP32-C3 - which runs a RiscV32 chip - you can just use the stock nightly Rust compiler, and a recent, stock Clang (as in Clang 11+)
  - (You can do this by issuing `rustup install nightly` and then `rustup default nightly` instead of installing/building the Rust & Clang ESP forks and switching to their `esp` toolchain as advised above)
- If using the custom Espressif Clang, make sure that you DON'T have a system Clang installed as well, because even if you have the Espressif one first on your `$PATH`, Bindgen will still pick the system one
  - A workaround that does not require uninstalling the system Clang is to do `export LIBCLANG_PATH=<path to the Espressif Clang lib directory>` prior to continuing the build process
- `cargo install ldproxy`
- Clone this repo: `git clone https://github.com/ivmarkov/rust-esp32-std-demo`
- Enter it: `cd rust-esp32-std-demo`
- Export two environment variables that would contain the SSID & password of your wireless network:
  - `export WIFI_SSID=<ssid>`
  - `export WIFI_PASS=<ssid>`
  -  export MQTT_URL=mqtt://username:password@example.org:1883
- To configure the demo for your particular board, please uncomment the relevant [Rust target for your board](https://github.com/ivmarkov/rust-esp32-std-demo/blob/main/.cargo/config.toml#L2) and comment the others. Alternatively, just append the `--target <target>` flag to all `cargo build` lines below.
- Build: `cargo build` or `cargo build --release`

## Flash

- `cargo install espflash`
- `espflash /dev/ttyUSB0 target/[xtensa-esp32-espidf|xtensa-esp32s2-espidf|riscv32imc-esp-espidf]/debug/rust-esp32-std-demo`
- Replace `dev/ttyUSB0` above with the USB port where you've connected the board

**NOTE**: The above commands do use [`espflash`](https://crates.io/crates/espflash) and NOT [`cargo espflash`](https://crates.io/crates/cargo-espflash), even though both can be installed via Cargo. `cargo espflash` is essentially `espflash` but it has some extra superpowers, like the capability to build the project before flashing, or to generate an ESP32 .BIN file from the built .ELF image.

## Alternative flashing

- You can also flash with the [esptool.py](https://github.com/espressif/esptool) utility which is part of the Espressif toolset
- Use the instructions below **only** if you have flashed successfully with `espflash` at least once, or else you might not have a valid bootloader and partition table!
- The instructions below only (re)flash the application image, as the (one and only) factory image starting from 0x10000 in the partition table!
- Install esptool using Python: `pip install esptool`
- (After each cargo build) Convert the elf image to binary: `esptool.py --chip [esp32|esp32s2|esp32c3] elf2image target/xtensa-esp32-espidf/debug/rust-esp32-std-demo`
- (After each cargo build) Flash the resulting binary: `esptool.py --chip [esp32|esp32s2|esp32c3] -p /dev/ttyUSB0 -b 460800 --before=default_reset --after=hard_reset write_flash --flash_mode dio --flash_freq 40m --flash_size 4MB 0x10000 target/xtensa-esp32-espidf/debug/rust-esp32-std-demo.bin`

## Monitor

- Once flashed, the board can be connected with any suitable serial monitor, e.g.:
  - ESPMonitor: `espmonitor /dev/ttyUSB0` (you need to `cargo install espmonitor` first)
  - Cargo PIO (this one **decodes stack traces**!): `cargo pio espidf monitor /dev/ttyUSB0` (you need to `cargo install cargo-pio` first)
    - Please run it from within the `rust-esp32-std-demo` project directory, or else the built ELF file will not be detected, and the stack traces will not be decoded!
  - Built-in Linux/MacOS screen: `screen /dev/ttyUSB0 115200` (use `Ctrl+A` and then type `:quit` to stop it)
  - Miniterm: `miniterm --raw /dev/ttyUSB0 115200`
