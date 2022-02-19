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

- If the app starts successfully, it should be listening on the printed IP address from the WiFi connection logs, port 80.

- Open a browser, and navigate to one of these:
  - `http://<printed-ip-address>`
  - `http://<printed-ip-address>/foo?key=value`
  - `http://<printed-ip-address>/bar`
  - `http://<printed-ip-address>/ulp` (ESP32-S2 only)

- The monitor should output more or less the following:
```
Hello, world from Rust!
More complex print [foo, bar]
Rust main thread: ...
This is thread number 0 ...
This is thread number 1 ...
This is thread number 2 ...
This is thread number 3 ...
This is thread number 4 ...
About to join the threads. If ESP-IDF was patched successfully, joining will NOT crash
Joins were successful.
I (4761) wifi:wifi driver task: 3ffc1d80, prio:23, stack:6656, core=0
I (4761) system_api: Base MAC address is not set, read default base MAC address from BLK0 of EFUSE
I (4761) system_api: Base MAC address is not set, read default base MAC address from BLK0 of EFUSE
I (4771) wifi:wifi firmware version: 3ea4c76
I (4771) wifi:config NVS flash: disabled
I (4781) wifi:config nano formating: disabled
I (4781) wifi:Init dynamic tx buffer num: 32
I (4791) wifi:Init data frame dynamic rx buffer num: 32
I (4791) wifi:Init management frame dynamic rx buffer num: 32
I (4801) wifi:Init management short buffer num: 32
I (4801) wifi:Init static rx buffer size: 1600
I (4811) wifi:Init static rx buffer num: 10
I (4811) wifi:Init dynamic rx buffer num: 32
I (4811) esp_idf_svc::wifi: Driver initialized
I (4821) esp_idf_svc::wifi: Event handlers registered
I (4821) esp_idf_svc::wifi: Initialization complete
I (4831) rust_esp32_std_demo: Wifi created
I (4831) esp_idf_svc::wifi: Setting configuration: Client(ClientConfiguration { ssid: "<your-ssid>", bssid: None, auth_method: WPA2Personal, password: "<your-pass>", ip_conf: Some(DHCP) })
I (4851) esp_idf_svc::wifi: Stopping
I (4861) esp_idf_svc::wifi: Disconnect requested
I (4861) esp_idf_svc::wifi: Stop requested
I (4871) esp_idf_svc::wifi: About to wait for status
I (4871) esp_idf_svc::wifi: Providing status: Status(Stopped, Stopped)
I (4881) esp_idf_svc::wifi: Waiting for status done - success
I (4881) esp_idf_svc::wifi: Stopped
I (4891) esp_idf_svc::wifi: Wifi mode STA set
I (4891) esp_idf_svc::wifi: Setting STA configuration: ClientConfiguration { ssid: "<your-ssid>", bssid: None, auth_method: WPA2Personal, password: "<your-pass>", ip_conf: Some(DHCP) }
I (4911) esp_idf_svc::wifi: Setting STA IP configuration: DHCP
I (4921) esp_idf_svc::wifi: STA netif allocated: 0x3ffc685c
I (4921) esp_idf_svc::wifi: STA IP configuration done
I (4931) esp_idf_svc::wifi: STA configuration done
I (4931) esp_idf_svc::wifi: Starting with status: Status(Starting, Stopped)
I (4941) esp_idf_svc::wifi: Status is of operating type, starting
I (5041) phy: phy_version: 4180, cb3948e, Sep 12 2019, 16:39:13, 0, 0
I (5041) wifi:mode : sta (f0:08:d1:77:68:f0)
I (5041) esp_idf_svc::wifi: Got wifi event: 2
I (5051) esp_idf_svc::wifi: Recconecting
I (5051) esp_idf_svc::wifi: Start requested
I (5051) esp_idf_svc::wifi: Set status: Status(Started(Connecting), Stopped)
I (5061) esp_idf_svc::wifi: About to wait for status with timeout 10s
I (5071) esp_idf_svc::wifi: Wifi event 2 handled
I (5091) esp_idf_svc::wifi: Providing status: Status(Started(Connecting), Stopped)
I (5171) wifi:new:<1,1>, old:<1,0>, ap:<255,255>, sta:<1,1>, prof:1
I (5941) wifi:state: init -> auth (b0)
I (5951) esp_idf_svc::wifi: Providing status: Status(Started(Connecting), Stopped)
I (5951) wifi:state: auth -> assoc (0)
I (5961) wifi:state: assoc -> run (10)
I (5981) wifi:connected with muci, aid = 1, channel 1, 40U, bssid = 08:55:31:2e:c3:cf
I (5981) wifi:security: WPA2-PSK, phy: bgn, rssi: -54
I (5981) wifi:pm start, type: 1

I (5991) esp_idf_svc::wifi: Got wifi event: 4
I (5991) esp_idf_svc::wifi: Set status: Status(Started(Connected(Waiting)), Stopped)
I (6001) esp_idf_svc::wifi: Wifi event 4 handled
I (6011) wifi:AP's beacon interval = 102400 us, DTIM period = 1
I (6451) esp_idf_svc::wifi: Providing status: Status(Started(Connected(Waiting)), Stopped)
I (6951) esp_idf_svc::wifi: Providing status: Status(Started(Connected(Waiting)), Stopped)
I (7451) esp_idf_svc::wifi: Providing status: Status(Started(Connected(Waiting)), Stopped)
I (7951) esp_idf_svc::wifi: Providing status: Status(Started(Connected(Waiting)), Stopped)
I (8221) esp_idf_svc::wifi: Got IP event: 0
I (8221) esp_idf_svc::wifi: Set status: Status(Started(Connected(Done(ClientSettings { ip: 192.168.10.155, subnet: Subnet { gateway: 192.168.10.1, mask: Mask(24) }, dns: None, secondary_dns: None }))), Stopped)
I (8231) esp_idf_svc::wifi: IP event 0 handled
I (8241) esp_netif_handlers: staSTA netif allocated:  ip: 192.168.10.155, mask: 255.255.255.0, gw: 192.168.10.1
I (8451) esp_idf_svc::wifi: Providing status: Status(Started(Connected(Done(ClientSettings { ip: 192.168.10.155, subnet: Subnet { gateway: 192.168.10.1, mask: Mask(24) }, dns: None, secondary_dns: None }))), Stopped)
I (8461) esp_idf_svc::wifi: Waiting for status done - success
I (8461) esp_idf_svc::wifi: Started
I (8471) esp_idf_svc::wifi: Configuration set
I (8471) rust_esp32_std_demo: Wifi configuration set, about to get status
I (8481) esp_idf_svc::wifi: Providing status: Status(Started(Connected(Done(ClientSettings { ip: 192.168.10.155, subnet: Subnet { gateway: 192.168.10.1, mask: Mask(24) }, dns: None, secondary_dns: None }))), Stopped)
I (8501) rust_esp32_std_demo: Wifi connected, about to do some pings
I (8511) esp_idf_svc::ping: About to run a summary ping 192.168.10.1 with configuration Configuration { count: 5, interval: 1s, timeout: 1s, data_size: 56, tos: 0 }
I (8521) esp_idf_svc::ping: Ping session established, got handle 0x3ffc767c
I (8531) esp_idf_svc::ping: Ping session started
I (8531) esp_idf_svc::ping: Waiting for the ping session to complete
I (8541) esp_idf_svc::ping: Ping success callback invoked
I (8551) esp_idf_svc::ping: From 192.168.10.1 icmp_seq=1 ttl=64 time=14ms bytes=64
I (9531) esp_idf_svc::ping: Ping success callback invoked
I (9531) esp_idf_svc::ping: From 192.168.10.1 icmp_seq=2 ttl=64 time=1ms bytes=64
I (10531) esp_idf_svc::ping: Ping success callback invoked
I (10531) esp_idf_svc::ping: From 192.168.10.1 icmp_seq=3 ttl=64 time=2ms bytes=64
I (11531) esp_idf_svc::ping: Ping success callback invoked
I (11531) esp_idf_svc::ping: From 192.168.10.1 icmp_seq=4 ttl=64 time=0ms bytes=64
I (12531) esp_idf_svc::ping: Ping success callback invoked
I (12531) esp_idf_svc::ping: From 192.168.10.1 icmp_seq=5 ttl=64 time=1ms bytes=64
I (13531) esp_idf_svc::ping: Ping end callback invoked
I (13531) esp_idf_svc::ping: 5 packets transmitted, 5 received, time 18ms
I (13531) esp_idf_svc::ping: Ping session stopped
I (13531) esp_idf_svc::ping: Ping session 0x3ffc767c removed
I (13541) rust_esp32_std_demo: Pinging done
I (13551) esp_idf_svc::httpd: Started Httpd IDF server with config Configuration { http_port: 80, https_port: 443 }
I (13561) esp_idf_svc::httpd: Registered Httpd IDF server handler Get for URI "/"
I (13561) esp_idf_svc::httpd: Registered Httpd IDF server handler Get for URI "/foo"
I (13571) esp_idf_svc::httpd: Registered Httpd IDF server handler Get for URI "/bar"
```
