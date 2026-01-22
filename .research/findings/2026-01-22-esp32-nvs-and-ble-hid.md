# ESP32 NVS Storage and BLE HID Keyboard Capabilities

**Date**: 2026-01-22
**Researcher**: Claude + User
**Status**: Complete

## Question/Goal

Research ESP32 capabilities for the Bitwarden hardware key PoC:
1. How to use NVS (Non-Volatile Storage) with esp-idf-svc Rust bindings for credential storage
2. How to implement BLE HID keyboard emulation on ESP32 with Rust
3. Storage capacity, limitations, and best practices
4. Available libraries and code examples

## Key Findings

### Finding 1: ESP32 NVS Storage with Rust

**Overview**: The esp-idf-svc crate provides type-safe Rust wrappers for ESP32 NVS (Non-Volatile Storage), enabling persistent key-value storage in flash memory.

**Core API (`EspNvs<T>`)**:
- Generic struct requiring a partition and namespace
- Supports typed operations: `get_u8()`, `set_u8()`, `get_i32()`, `set_i32()`, etc.
- String operations: `get_str()`, `set_str()` with buffer management
- Blob operations: `get_blob()`, `set_blob()` for binary data
- Key management: `contains()`, `remove()`, `keys()` iterator

**Initialization Pattern**:
```rust
use esp_idf_svc::nvs::*;

// Take the default NVS partition
let nvs_default_partition: EspNvsPartition<NvsDefault> =
    EspDefaultNvsPartition::take().unwrap();

// Create NVS handle with namespace
let nvs = EspNvs::new(nvs_default_partition, "namespace", true).unwrap();

// Write/read operations
nvs.set_u8("counter", 42)?;
let value = nvs.get_u8("counter")?.unwrap_or(0);
```

**Storage Capacity and Limitations**:
- **Minimum partition size**: 12KB (3 sectors × 4KB)
- **Recommended size**: 12KB to 64KB
- **String size limit**: 4000 bytes (including null terminator)
- **Blob size limit**: 508,000 bytes or 97.6% of partition size - 4000 bytes, whichever is lower
- **Key length limit**: 15 characters (ASCII)
- **Actual usable space**: ~83% of partition size (17% overhead for management)
- **Best for**: Many small values rather than large blobs
- **For larger data**: Use FAT or SPIFFS filesystem instead

**Data Types Supported**:
- Unsigned integers: u8, u16, u32, u64
- Signed integers: i8, i16, i32, i64
- Strings: UTF-8 encoded
- Blobs: Raw binary data

**Error Handling**:
- Getters return `Result<Option<T>, EspError>` (None indicates missing key)
- Setters return `Result<(), EspError>` or `Result<bool, EspError>`
- Thread-safe: Implements `Send` and `Sync`

**Working Example**:
```rust
use esp_idf_svc::nvs::*;

fn nvs_example() -> anyhow::Result<()> {
    let nvs_partition = EspDefaultNvsPartition::take()?;
    let nvs = EspNvs::new(nvs_partition, "credentials", true)?;

    // Store string (requires buffer for reading)
    nvs.set_str("username", "user@example.com")?;

    const MAX_LEN: usize = 100;
    let mut buffer = [0u8; MAX_LEN];
    let username = nvs.get_str("username", &mut buffer)?
        .map(|s| s.trim_end_matches(char::from(0)));

    // Store binary blob
    let password_hash = [0xAB, 0xCD, 0xEF];
    nvs.set_blob("password", &password_hash)?;

    let mut blob_buf = [0u8; 32];
    let hash = nvs.get_blob("password", &mut blob_buf)?;

    Ok(())
}
```

**Sources**:
- [esp-idf-svc GitHub Repository](https://github.com/esp-rs/esp-idf-svc)
- [EspNvs API Documentation](https://docs.esp-rs.org/esp-idf-svc/esp_idf_svc/nvs/struct.EspNvs.html)
- [ESP-IDF NVS Documentation](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/storage/nvs_flash.html)
- [Wokwi NVS Example](https://wokwi.com/projects/367961293345482753)
- [esp-idf-svc Issue #268](https://github.com/esp-rs/esp-idf-svc/issues/268)
- [esp-idf-svc nvs.rs Source](https://github.com/esp-rs/esp-idf-svc/blob/master/src/nvs.rs)

### Finding 2: NVS Encryption for Secure Credential Storage

**Overview**: ESP32 supports NVS encryption using XTS-AES encryption (IEEE P1619 standard) for secure storage of sensitive data like credentials.

**Security Features**:
- XTS-AES encryption for NVS partitions
- Two encryption schemes available:
  1. **Flash Encryption-based**: Keys stored in encrypted partition (requires flash encryption enabled)
  2. **HMAC-based** (ESP32-S2/S3/C6): Keys derived from eFuse HMAC key at runtime (no keys stored in flash)

**Prerequisites**:
- Flash encryption must be enabled (for scheme 1)
- Dedicated key partition (type: data, subtype: nvs_keys, minimum 4KB, marked as encrypted)

**Why Important for Credentials**:
- WiFi driver stores SSID/passphrase in default NVS partition
- Default ESP-IDF components write device-specific data to NVS
- **Recommended**: Always use NVS encryption for credential storage

**API Usage**:
```rust
// Applications use:
// 1. nvs_flash_read_security_cfg() or nvs_flash_generate_keys()
// 2. nvs_flash_secure_init() or nvs_flash_secure_init_partition()
```

**Best Practices**:
- Enable flash encryption for production devices
- Use HMAC-based encryption on supported chips (ESP32-S2/S3/C6)
- Never embed sensitive information in firmware images
- Ensure physical access cannot reveal unencrypted data

**Sources**:
- [NVS Encryption - ESP32](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/storage/nvs_encryption.html)
- [How to encrypt NVS on ESP32 - DEV](https://dev.to/kkentzo/how-to-encrypt-the-nvs-volume-on-the-esp32-4n9k)
- [Secure ESP32 Device Provisioning](https://medium.com/lifeomic/secure-esp32-device-provisioning-with-hardware-security-ccb72ea5c326)
- [ESP32 Security Overview](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/security/security.html)

### Finding 3: BLE HID Keyboard with esp32-nimble

**Overview**: The `esp32-nimble` crate provides Rust wrappers for the NimBLE Bluetooth stack, enabling BLE HID keyboard implementation on ESP32.

**Library**: esp32-nimble
- GitHub: [taks/esp32-nimble](https://github.com/taks/esp32-nimble)
- Documentation: [esp32_nimble docs](https://taks.github.io/esp32-nimble/esp32_nimble/index.html)
- Version: 0.11.1 (latest release April 2025)
- License: Apache 2.0

**SDK Configuration** (sdkconfig.defaults):
```
CONFIG_BT_ENABLED=y
CONFIG_BT_BLE_ENABLED=y
CONFIG_BT_BLUEDROID_ENABLED=n
CONFIG_BT_NIMBLE_ENABLED=y
CONFIG_BT_NIMBLE_NVS_PERSIST=y  # For iOS reconnection
```

**Advantages of NimBLE**:
- Significant RAM and flash memory savings vs Bluedroid stack
- Better suited for resource-constrained applications
- Active community and Rust support

**Basic Structure**:
```rust
use esp32_nimble::{BLEDevice, BLEServer, BLEAdvertising};

// Initialize BLE
let ble_device = BLEDevice::take();
ble_device.security()
    .set_auth(AuthReq::Bond)  // Enable bonding
    .set_passkey(123456)      // Optional passkey
    .set_io_cap(SecurityIOCap::NoInputNoOutput);

// Create server and services
let server = ble_device.get_server();
// Add HID service and characteristics here

// Start advertising
let advertising = ble_device.get_advertising();
advertising.start()?;
```

**iOS Compatibility**:
- Set `CONFIG_BT_NIMBLE_NVS_PERSIST=y` for persistent bonding
- Enables automatic reconnection without re-pairing
- Requires security configuration with bonding and RPA resolution

**Security Configuration**:
```rust
device.security()
    .set_auth(AuthReq::Bond)  // Stores keys for reconnection
    .resolve_rpa()            // Handles iOS dynamic addresses
    .set_passkey(format!("{:0>6}", pkey))  // 6 digits with padding
```

**Key Requirements for BLE Security**:
1. **Bonding**: Stores encryption keys for future connections
2. **RPA Resolution**: Manages resolvable private addresses (iOS requirement)
3. **Passkeys**: Must be exactly 6 digits

**Learning Resources**:
- [Embedded Rust BLE tutorials](https://blog.theembeddedrustacean.com/)
- Code examples on [apollolabs ESP32C3 repo](https://github.com/apollolabs)
- [ESP32-NimBLE-For-Dummies](https://github.com/Zeni241/ESP32-NimbleBLE-For-Dummies)

**Sources**:
- [esp32-nimble GitHub](https://github.com/taks/esp32-nimble)
- [esp32_nimble Documentation](https://taks.github.io/esp32-nimble/esp32_nimble/index.html)
- [Embedded Rust BLE Scanner](https://dev.to/theembeddedrustacean/embedded-rust-bluetooth-on-esp-ble-scanner-1gb7)
- [Embedded Rust Secure BLE Client](https://blog.theembeddedrustacean.com/embedded-rust-bluetooth-on-esp-secure-ble-client)

### Finding 4: BLE HID Protocol Implementation

**BLE HID Service Specification**:
- **Service UUID**: 0x1812 (HID Service - Bluetooth SIG standard)
- **Appearance Value**: 0xC103 (keyboard)
- **Required Additional Services**:
  - Battery Service
  - Device Information Service

**GATT Structure**:
- **Protocol Mode characteristic**: Boot mode or Report mode (default)
- **Report Map characteristic**: HID Report Descriptor (USB HID format)
  - Maximum length: 512 octets
  - Defines format for Input/Output/Feature reports
- **Report characteristics**: With Report Reference descriptors
- **Boot Keyboard Input Report**: Required for keyboard devices

**HID Report Descriptor**:
- Uses USB HID specification format
- Returned when Report Map characteristic is read
- Defines all possible report formats

**Example Reference Implementation**:
A bare-metal Rust example exists: [esp32c3-ble-hid](https://github.com/bjoernQ/esp32c3-ble-hid)
- Tested on Android and Windows 11
- Sends "esp32" when boot button pressed
- **Known limitation**: LTK (Long Term Key) not persisted - requires re-pairing after reboot

**Sources**:
- [BLE HID Keyboard - Silicon Labs](https://docs.silabs.com/bluetooth/2.13/code-examples/applications/ble-hid-keyboard)
- [HID Service Specification PDF](https://devzone.nordicsemi.com/cfs-file/__key/support-attachments/beef5d1b77644c448dabff31668f3a47-d2e31a4fe5fd4955b77461e3188e06af/HIDS_5F00_SPEC_5F00_V10.pdf)
- [BLE HID Device Implementation](https://circuitlabs.net/ble-hid-device-implementation/)
- [Bluetooth HID Device Design](https://novelbits.io/bluetooth-hid-device-design-example-project/)
- [esp32c3-ble-hid GitHub](https://github.com/bjoernQ/esp32c3-ble-hid/)

### Finding 5: Arduino/C++ Reference Libraries (Non-Rust)

While not directly usable in Rust, these mature libraries provide excellent reference for understanding BLE HID implementation:

**ESP32-BLE-Keyboard** (Arduino):
- GitHub: [T-vK/ESP32-BLE-Keyboard](https://github.com/T-vK/ESP32-BLE-Keyboard)
- Most popular ESP32 BLE keyboard library
- Well-documented with examples
- Useful for understanding HID report descriptors and keyboard matrix

**ESP32-BLE-Combo**:
- GitHub: [BlynkGO/ESP32-BLE-Combo](https://github.com/BlynkGO/ESP32-BLE-Combo)
- Supports keyboard + mouse + media keys
- Good reference for multi-function HID devices

**ESP32 Mouse and Keyboard**:
- GitHub: [asterics/esp32_mouse_keyboard](https://github.com/asterics/esp32_mouse_keyboard)
- HID over GATT implementation
- Includes serial API (similar to Adafruit EZKey HID)

**Value**: These can be referenced for:
- HID report descriptors
- GATT service structure
- Pairing/bonding workflows
- Keystroke encoding

**Sources**:
- [ESP32-BLE-Keyboard](https://github.com/T-vK/ESP32-BLE-Keyboard)
- [ESP32 Mouse and Keyboard](https://github.com/asterics/esp32_mouse_keyboard)
- [ESP32-BLE-Combo](https://github.com/BlynkGO/ESP32-BLE-Combo)
- [Emulating a Bluetooth Keyboard - Hackaday](https://hackaday.com/2020/02/13/emulating-a-bluetooth-keyboard-with-the-esp32/)

## Implications for Our Project

### NVS Storage for Credentials

**Recommended Approach**:
1. Use `esp-idf-svc` NVS API with encrypted partition
2. Store Bitwarden credentials as encrypted blobs
3. Keep credential database compact (use 16-64KB partition)
4. Consider string length limits (4000 bytes max per value)

**Security Considerations**:
- Enable flash encryption in production
- Use HMAC-based encryption if targeting ESP32-S2/S3/C6
- Never store plaintext passwords
- Consider additional encryption layer for credential blobs

**Storage Strategy**:
- Use separate namespace for credentials ("bitwarden_creds")
- Store metadata separately (last_sync, user_email, etc.)
- Implement proper error handling for NVS operations
- Consider migration strategy if storage format changes

### BLE HID Implementation

**Recommended Approach**:
1. Use `esp32-nimble` crate for BLE stack
2. Implement HID Service (UUID 0x1812) with proper descriptors
3. Add Battery Service and Device Information Service
4. Enable bonding with passkey for security

**Implementation Path**:
1. Study the bare-metal example: [esp32c3-ble-hid](https://github.com/bjoernQ/esp32c3-ble-hid)
2. Reference Arduino libraries for HID report descriptors
3. Follow "The Embedded Rustacean" tutorials for esp32-nimble usage
4. Implement persistent bonding (solve the LTK persistence issue)

**Security Requirements**:
- Enable `CONFIG_BT_NIMBLE_NVS_PERSIST` for bonding persistence
- Use passkey authentication (6 digits)
- Implement RPA resolution for iOS compatibility
- Consider MITM protection requirements

### Integration Challenges

**Key Challenges to Address**:
1. **Bonding Persistence**: The reference implementation doesn't persist LTK
   - Need to store bonding keys in NVS
   - Implement automatic reconnection
2. **Memory Constraints**: ESP32 has limited RAM
   - NimBLE uses less memory than Bluedroid (good choice)
   - Monitor stack size (CONFIG_BT_NIMBLE_HOST_TASK_STACK_SIZE)
3. **HID Report Descriptor**: Need to define keyboard report format
   - Reference Arduino libraries for descriptors
   - Keep descriptor compact (512 byte limit)
4. **Multi-tasking**: GUI + BLE + NVS operations
   - Use esp-idf task management
   - Consider priority and scheduling

## Recommendations

### Phase 1: NVS Storage Implementation
1. Add NVS partition to partition table (16KB, encrypted)
2. Implement credential storage module using `esp-idf-svc::nvs`
3. Create tests for storing/retrieving credentials
4. Implement encryption wrapper for credential blobs
5. Enable flash encryption for production

### Phase 2: BLE HID Proof of Concept
1. Add `esp32-nimble` dependency to Cargo.toml
2. Configure sdkconfig.defaults for NimBLE
3. Implement basic BLE advertising and GATT server
4. Add HID service with keyboard report descriptor
5. Test basic keystroke sending (hardcoded test strings)

### Phase 3: Integration
1. Implement bonding persistence in NVS
2. Create UI for BLE pairing flow (display passkey on OLED)
3. Integrate credential retrieval with keystroke transmission
4. Add battery service reporting
5. Test on multiple platforms (Windows, macOS, iOS, Android)

### Phase 4: Security & Polish
1. Enable flash encryption
2. Implement secure credential storage with additional encryption
3. Add timeout for BLE connections
4. Implement proper key clearing on device lock
5. Security audit of credential handling

## Open Questions

1. **Flash Encryption**: Do we need to enable flash encryption, or is NVS encryption sufficient?
   - Answer: For production, enable both. Flash encryption protects firmware and NVS keys.

2. **Key Derivation**: Should we derive encryption keys from a master password?
   - Consider PBKDF2 or similar for additional security layer.

3. **Bonding Storage**: Where to persist LTK and other bonding information?
   - Store in separate NVS namespace ("ble_bonding")

4. **Cross-Platform Testing**: Which platforms to prioritize?
   - Start with desktop (Windows/macOS), then mobile (iOS critical for passkey input)

5. **Keystroke Timing**: How to handle typing speed and special characters?
   - Research HID scan codes and timing requirements

6. **Battery Monitoring**: How to report battery level through BLE?
   - ESP32 can measure VCC with ADC (if connected)

## Sources Summary

### ESP32 NVS Resources
- [esp-idf-svc GitHub Repository](https://github.com/esp-rs/esp-idf-svc)
- [EspNvs API Documentation](https://docs.esp-rs.org/esp-idf-svc/esp_idf_svc/nvs/struct.EspNvs.html)
- [ESP-IDF NVS Documentation](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/storage/nvs_flash.html)
- [NVS Encryption Documentation](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/storage/nvs_encryption.html)
- [ESP32 NVS Data Storage Guide](https://medium.com/engineering-iot/nvs-data-storage-and-reading-in-esp32-a-comprehensive-guide-12bdbc6325ac)
- [Custom Partition Tables](https://developer.espressif.com/blog/how-to-use-custom-partition-tables-on-esp32/)

### BLE HID Resources
- [esp32-nimble GitHub](https://github.com/taks/esp32-nimble)
- [esp32-nimble Documentation](https://taks.github.io/esp32-nimble/esp32_nimble/index.html)
- [esp32c3-ble-hid Example](https://github.com/bjoernQ/esp32c3-ble-hid/)
- [ESP32-BLE-Keyboard (Arduino)](https://github.com/T-vK/ESP32-BLE-Keyboard)
- [NimBLE HID Example (C)](https://github.com/olegos76/nimble_kbdhid_example)

### BLE Security & Tutorials
- [Embedded Rust BLE Scanner](https://dev.to/theembeddedrustacean/embedded-rust-bluetooth-on-esp-ble-scanner-1gb7)
- [Embedded Rust Secure BLE Client](https://blog.theembeddedrustacean.com/embedded-rust-bluetooth-on-esp-secure-ble-client)
- [Embedded Rust Secure BLE Server](https://dev.to/theembeddedrustacean/embedded-rust-bluetooth-on-esp-secure-ble-server-3604)

### BLE HID Specifications
- [BLE HID Keyboard - Silicon Labs](https://docs.silabs.com/bluetooth/2.13/code-examples/applications/ble-hid-keyboard)
- [HID Service Specification](https://devzone.nordicsemi.com/cfs-file/__key/support-attachments/beef5d1b77644c448dabff31668f3a47-d2e31a4fe5fd4955b77461e3188e06af/HIDS_5F00_SPEC_5F00_V10.pdf)
- [BLE HID Device Implementation](https://circuitlabs.net/ble-hid-device-implementation/)
- [Bluetooth HID Device Design](https://novelbits.io/bluetooth-hid-device-design-example-project/)
