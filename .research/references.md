# Research References

Collection of useful links, documentation, papers, and external resources for this project.

## ESP32 & esp-rs

### Official Documentation
- [esp-rs Book](https://esp-rs.github.io/book/) - Official guide for Rust on ESP32
- [ESP-IDF Documentation](https://docs.espressif.com/projects/esp-idf/en/latest/) - ESP32 development framework
- [esp-rs/esp-idf-template](https://github.com/esp-rs/esp-idf-template) - Project template used for this project

### Tools & Setup
- [espup](https://github.com/esp-rs/espup) - Tool for installing esp-rs toolchain
- [espflash](https://github.com/esp-rs/espflash) - Tool for flashing ESP32 devices

## Hardware

### Adafruit HUZZAH32
- [Product Page](https://www.adafruit.com/product/3405) - HUZZAH32 ESP32 Feather board
- [Pinout Diagram](https://learn.adafruit.com/adafruit-huzzah32-esp32-feather/pinouts)

### SSD1306 OLED Display
- [Adafruit OLED Feather Wing](https://www.adafruit.com/product/2900) - 128x32 OLED display
- [SSD1306 Datasheet](https://cdn-shop.adafruit.com/datasheets/SSD1306.pdf)

## Embedded GUI Development

### Frameworks & Libraries
- [embedded-graphics](https://github.com/embedded-graphics/embedded-graphics) - 2D graphics library for embedded devices
- [lvgl](https://lvgl.io/) - Light and Versatile Graphics Library (if considered)

## Bitwarden

### API & Protocol
- [Bitwarden API Documentation](https://bitwarden.com/help/api/)
- [Bitwarden CLI](https://github.com/bitwarden/clients/tree/master/apps/cli) - Reference implementation

## Security & Cryptography

### General Resources
- [Rust Crypto](https://github.com/RustCrypto) - Cryptography implementations in Rust
- [ESP32 Secure Boot](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/security/secure-boot-v2.html)

### ESP32 Secure Storage
- [NVS Encryption](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/storage/nvs_encryption.html) - Non-volatile storage encryption
- [ESP32 Security Overview](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/security/security.html) - Comprehensive security features
- [Flash Encryption](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/security/flash-encryption.html) - Protecting firmware and data

## Storage & Persistence

### NVS (Non-Volatile Storage)
- [NVS Flash Library](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/storage/nvs_flash.html) - ESP-IDF NVS documentation
- [esp-idf-svc NVS Module](https://docs.esp-rs.org/esp-idf-svc/esp_idf_svc/nvs/index.html) - Rust NVS API
- [EspNvs Documentation](https://docs.esp-rs.org/esp-idf-svc/esp_idf_svc/nvs/struct.EspNvs.html) - Type-safe NVS wrapper
- [Custom Partition Tables](https://developer.espressif.com/blog/how-to-use-custom-partition-tables-on-esp32/) - Partition management guide

## Bluetooth Low Energy (BLE)

### BLE HID Resources
- [esp32-nimble](https://github.com/taks/esp32-nimble) - Rust wrapper for NimBLE stack
- [esp32-nimble Documentation](https://taks.github.io/esp32-nimble/esp32_nimble/index.html) - API documentation
- [esp32c3-ble-hid Example](https://github.com/bjoernQ/esp32c3-ble-hid/) - Bare-metal BLE HID keyboard in Rust
- [ESP32-BLE-Keyboard](https://github.com/T-vK/ESP32-BLE-Keyboard) - Arduino reference implementation
- [NimBLE HID Example (C)](https://github.com/olegos76/nimble_kbdhid_example) - C implementation reference

### BLE Specifications
- [HID Service Specification](https://devzone.nordicsemi.com/cfs-file/__key/support-attachments/beef5d1b77644c448dabff31668f3a47-d2e31a4fe5fd4955b77461e3188e06af/HIDS_5F00_SPEC_5F00_V10.pdf) - Official HID over GATT spec
- [BLE HID Implementation Guide](https://circuitlabs.net/ble-hid-device-implementation/) - Implementation walkthrough
- [Bluetooth HID Design Guide](https://novelbits.io/bluetooth-hid-device-design-example-project/) - Comprehensive design guide

### BLE Tutorials (Rust on ESP32)
- [The Embedded Rustacean Blog](https://blog.theembeddedrustacean.com/) - BLE tutorials for ESP32 in Rust
- [Embedded Rust BLE Scanner](https://dev.to/theembeddedrustacean/embedded-rust-bluetooth-on-esp-ble-scanner-1gb7) - BLE scanner tutorial
- [Secure BLE Client Tutorial](https://blog.theembeddedrustacean.com/embedded-rust-bluetooth-on-esp-secure-ble-client) - Security and pairing
- [Secure BLE Server Tutorial](https://dev.to/theembeddedrustacean/embedded-rust-bluetooth-on-esp-secure-ble-server-3604) - Server-side security

## Rust Embedded

### General Resources
- [The Embedded Rust Book](https://rust-embedded.github.io/book/)
- [Awesome Embedded Rust](https://github.com/rust-embedded/awesome-embedded-rust)

## Notes

- Add new references as you discover them during research
- Organize by category for easy navigation
- Include direct links when possible
- Note the date when particularly time-sensitive resources are added
