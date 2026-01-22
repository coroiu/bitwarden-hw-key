# Technical Design: Phase 1 - Keyboard Emulation PoC

**Date**: 2026-01-22
**Status**: Active
**Phase**: 1 of 4

## Overview

This document describes the technical architecture for Phase 1 of the Bitwarden Hardware Key proof-of-concept. The goal is to validate that browsing and using credentials on a 128x32 OLED display with 3-button navigation is practical and usable.

## Goals

### Primary Goal
Prove that the hardware form factor (128x32 display + 3 buttons) is viable for credential management by implementing end-to-end keyboard emulation flow.

### Success Criteria
- ✅ User can sync credentials from Web Vault to device (desktop emulator or ESP32)
- ✅ User can browse 50+ credentials with smooth scrolling
- ✅ User can select a credential and have it typed into a login form
- ✅ Credentials persist across device restarts
- ✅ Entire flow feels natural and usable

### Non-Goals (Phase 2+)
- ❌ FIDO2/CTAP2 passkey support
- ❌ Encryption at rest (beyond basic NVS encryption)
- ❌ OTP/TOTP generation
- ❌ Advanced search/filtering
- ❌ Multi-user support
- ❌ WiFi connectivity

## Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────┐
│                     Web Vault (Angular)                      │
│  - User authentication (existing)                            │
│  - Credential export to CBOR                                 │
│  - HTTP client (desktop) or Web Bluetooth (ESP32)           │
└────────────┬─────────────────────────────────────────────────┘
             │
             │ Desktop: HTTP POST /api/sync
             │ ESP32:   BLE Write to credential_sync_char
             │ Format:  CBOR-encoded credential list
             │
             ▼
┌──────────────────────────────────────────────────────────────┐
│              Device (Desktop Emulator or ESP32)              │
│                                                              │
│  ┌────────────────────┐  ┌──────────────────────────────┐  │
│  │  Transport Layer   │  │  Storage Layer               │  │
│  │  - HTTP (desktop)  │─▶│  - NVS (ESP32)               │  │
│  │  - BLE (ESP32)     │  │  - JSON file (desktop)       │  │
│  └────────────────────┘  │  - Encrypted                 │  │
│                          │  - 16KB partition            │  │
│                          └──────────┬───────────────────┘  │
│                                     │                       │
│  ┌──────────────────────────────────▼───────────────────┐  │
│  │  Application Layer                                    │  │
│  │  ┌────────────────┐  ┌──────────────┐               │  │
│  │  │ Credential Mgr │  │  GUI System  │               │  │
│  │  │ - CBOR decode  │  │  - Document  │               │  │
│  │  │ - Validation   │  │  - Focus mgmt│               │  │
│  │  │ - Indexing     │  │  - Rendering │               │  │
│  │  └────────┬───────┘  └──────┬───────┘               │  │
│  │           │                  │                        │  │
│  │           ▼                  ▼                        │  │
│  │  ┌───────────────────────────────────────┐           │  │
│  │  │   Credential List View                │           │  │
│  │  │   - VerticalMenu component            │           │  │
│  │  │   - Scrolling & focus                 │           │  │
│  │  │   - Item selection                    │           │  │
│  │  └───────────────┬───────────────────────┘           │  │
│  │                  │ on_activation                     │  │
│  │                  ▼                        │           │  │
│  │  ┌───────────────────────────────────────┐           │  │
│  │  │   Credential Detail View              │           │  │
│  │  │   - Show name, username, URI          │           │  │
│  │  │   - Password hidden by default        │           │  │
│  │  │   - "Type credentials" action         │           │  │
│  │  └───────────────┬───────────────────────┘           │  │
│  └──────────────────┼───────────────────────────────────┘  │
│                     │                                       │
│                     ▼                                       │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Output Layer                                        │  │
│  │  - Desktop: Keyboard input simulation (enigo?)      │  │
│  │  - ESP32:   BLE HID keyboard (esp32-nimble)         │  │
│  └──────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Component Design

### 1. Credential Sync

#### Data Model

```rust
// src/credentials/mod.rs

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: Uuid,
    pub name: String,           // "GitHub"
    pub username: String,       // "user@example.com"
    pub password: String,       // Plaintext for now
    pub uri: Option<String>,    // "https://github.com"
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncRequest {
    pub credentials: Vec<Credential>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResponse {
    pub status: String,
    pub synced: usize,
    pub total_bytes: usize,
}
```

#### CBOR Encoding

Use `ciborium` for CBOR encoding/decoding:

```rust
// Encode credentials to CBOR
let sync_request = SyncRequest { credentials };
let mut cbor_bytes = Vec::new();
ciborium::into_writer(&sync_request, &mut cbor_bytes)?;

// Decode CBOR to credentials
let sync_request: SyncRequest = ciborium::from_reader(&cbor_bytes[..])?;
```

**Benefits of CBOR**:
- 30-50% smaller than JSON
- Binary format, faster parsing
- Built-in type safety
- Standard (RFC 8949)

#### Storage Format

**Desktop** (JSON file for easy debugging):
```json
// ~/.bitwarden-hw-key/credentials.json
{
  "credentials": [
    {
      "id": "123e4567-e89b-12d3-a456-426614174000",
      "name": "GitHub",
      "username": "user@example.com",
      "password": "secret123",
      "uri": "https://github.com",
      "notes": null
    }
  ],
  "last_sync": "2026-01-22T10:30:00Z"
}
```

**ESP32** (NVS):
```
Namespace: "bw_creds"
Keys:
  - count: u32 (number of credentials)
  - cred_0: blob (CBOR-encoded Credential)
  - cred_1: blob (CBOR-encoded Credential)
  - ...
  - last_sync: str (ISO 8601 timestamp)
```

### 2. Desktop Emulator HTTP Server

#### Implementation

```rust
// src/desktop/http_server.rs

use tiny_http::{Server, Response, Method, StatusCode};
use std::sync::{Arc, Mutex};
use ciborium;

pub struct SyncServer {
    server: Server,
    credentials: Arc<Mutex<Vec<Credential>>>,
}

impl SyncServer {
    pub fn new(addr: &str) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            server: Server::http(addr)?,
            credentials: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn handle_request(&self) -> Result<(), Box<dyn Error>> {
        let request = self.server.recv()?;

        match (request.method(), request.url()) {
            (&Method::Post, "/api/sync") => {
                self.handle_sync(request)
            }
            (&Method::Get, "/api/status") => {
                self.handle_status(request)
            }
            (&Method::Post, "/api/clear") => {
                self.handle_clear(request)
            }
            _ => {
                request.respond(Response::from_string("Not Found")
                    .with_status_code(StatusCode(404)))
            }
        }
    }

    fn handle_sync(&self, mut request: tiny_http::Request) -> Result<(), Box<dyn Error>> {
        // Parse CBOR body
        let sync_req: SyncRequest = ciborium::from_reader(request.as_reader())?;

        // Store credentials
        let mut creds = self.credentials.lock().unwrap();
        *creds = sync_req.credentials;

        // Respond
        let response = SyncResponse {
            status: "success".to_string(),
            synced: creds.len(),
            total_bytes: 0, // TODO: calculate
        };
        let json = serde_json::to_string(&response)?;
        request.respond(Response::from_string(json)
            .with_header("Content-Type: application/json".parse().unwrap())
            .with_header("Access-Control-Allow-Origin: http://localhost:4200".parse().unwrap()))
    }

    pub fn get_credentials(&self) -> Vec<Credential> {
        self.credentials.lock().unwrap().clone()
    }
}
```

#### Main Loop Integration

```rust
// src/bin/desktop.rs

fn main() {
    // Start HTTP server in background thread
    let server = SyncServer::new("127.0.0.1:8080").unwrap();
    let credentials = server.credentials.clone();

    std::thread::spawn(move || {
        loop {
            server.handle_request().ok();
        }
    });

    println!("Desktop emulator running on http://localhost:8080");

    // Main GUI loop
    let mut document = simple_view::create_view(WIDTH as u32, HEIGHT as u32);
    let mut last_cred_count = 0;

    while window.is_open() {
        // Check for new credentials
        let creds = credentials.lock().unwrap();
        if creds.len() != last_cred_count {
            document.update_credentials(&creds);
            last_cred_count = creds.len();
        }
        drop(creds);

        // ... existing input/render loop
    }
}
```

### 3. ESP32 NVS Storage

#### Implementation

```rust
// src/esp32/storage.rs

use esp_idf_svc::nvs::*;
use crate::credentials::Credential;

pub struct CredentialStorage {
    nvs: EspNvs<NvsDefault>,
}

impl CredentialStorage {
    pub fn new(partition: Arc<EspDefaultNvsPartition>) -> Result<Self, EspError> {
        let nvs = EspNvs::new(partition, "bw_creds", true)?;
        Ok(Self { nvs })
    }

    pub fn store_credentials(&mut self, credentials: &[Credential]) -> Result<(), EspError> {
        // Store count
        self.nvs.set_u32("count", credentials.len() as u32)?;

        // Store each credential as CBOR blob
        for (i, cred) in credentials.iter().enumerate() {
            let mut cbor_bytes = Vec::new();
            ciborium::into_writer(cred, &mut cbor_bytes)
                .map_err(|_| EspError::from_infallible::<ESP_ERR_INVALID_ARG>())?;

            let key = format!("cred_{}", i);
            self.nvs.set_blob(&key, &cbor_bytes)?;
        }

        Ok(())
    }

    pub fn load_credentials(&self) -> Result<Vec<Credential>, EspError> {
        let count = self.nvs.get_u32("count")?.unwrap_or(0) as usize;
        let mut credentials = Vec::with_capacity(count);

        for i in 0..count {
            let key = format!("cred_{}", i);
            let mut buffer = vec![0u8; 4096]; // Max credential size

            if let Some(len) = self.nvs.get_blob(&key, &mut buffer)? {
                let cred: Credential = ciborium::from_reader(&buffer[..len])
                    .map_err(|_| EspError::from_infallible::<ESP_ERR_INVALID_ARG>())?;
                credentials.push(cred);
            }
        }

        Ok(credentials)
    }

    pub fn clear(&mut self) -> Result<(), EspError> {
        // Clear all keys (NVS doesn't have "clear all")
        let count = self.nvs.get_u32("count")?.unwrap_or(0) as usize;
        for i in 0..count {
            let key = format!("cred_{}", i);
            self.nvs.remove(&key)?;
        }
        self.nvs.remove("count")?;
        Ok(())
    }
}
```

#### Flash Configuration

Update `partitions.csv` to allocate NVS space:

```csv
# Name,     Type, SubType, Offset,  Size,     Flags
nvs,        data, nvs,     0x9000,  0x4000,  encrypted
phy_init,   data, phy,     0xd000,  0x1000,
factory,    app,  factory, 0x10000, 1M,
```

Enable NVS encryption in `sdkconfig.defaults`:

```
CONFIG_NVS_ENCRYPTION=y
CONFIG_SECURE_FLASH_ENC_ENABLED=y
```

### 4. BLE HID Keyboard (ESP32 Only)

#### Implementation

```rust
// src/esp32/ble_keyboard.rs

use esp32_nimble::{BLEDevice, BLEHIDDevice, BLECharacteristic};

pub struct BleKeyboard {
    hid_device: BLEHIDDevice,
    input_report: BLECharacteristic,
}

impl BleKeyboard {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let device = BLEDevice::take();
        device.set_device_name("Bitwarden Key")?;

        let server = device.get_server();
        let mut hid_device = BLEHIDDevice::new(server);

        // Set HID report descriptor for keyboard
        hid_device.report_map(KEYBOARD_REPORT_MAP);
        hid_device.manufacturer("Bitwarden");
        hid_device.pnp(0x02, 0x05ac, 0x820a, 0x0210); // Apple VID/PID for compatibility

        // Battery service
        hid_device.battery_level(100);

        // Start advertising
        let advertising = device.get_advertising();
        advertising.add_service_uuid(hid_device.hid_service().uuid());
        advertising.start()?;

        Ok(Self {
            hid_device,
            input_report: hid_device.input_report(1), // Report ID 1
        })
    }

    pub fn type_string(&mut self, text: &str) -> Result<(), Box<dyn Error>> {
        for c in text.chars() {
            let (keycode, modifier) = char_to_keycode(c);

            // Press key
            let report = [modifier, 0, keycode, 0, 0, 0, 0, 0];
            self.input_report.set_value(&report);
            self.input_report.notify();

            // Release key
            let release = [0, 0, 0, 0, 0, 0, 0, 0];
            self.input_report.set_value(&release);
            self.input_report.notify();

            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    pub fn type_credentials(&mut self, username: &str, password: &str) -> Result<(), Box<dyn Error>> {
        self.type_string(username)?;
        self.press_key(HID_KEY_TAB)?;
        self.type_string(password)?;
        self.press_key(HID_KEY_RETURN)?;
        Ok(())
    }
}

// HID Report Descriptor (standard keyboard)
const KEYBOARD_REPORT_MAP: &[u8] = &[
    0x05, 0x01,       // Usage Page (Generic Desktop)
    0x09, 0x06,       // Usage (Keyboard)
    0xA1, 0x01,       // Collection (Application)
    0x85, 0x01,       //   Report ID (1)
    0x05, 0x07,       //   Usage Page (Key Codes)
    0x19, 0xE0,       //   Usage Minimum (224)
    0x29, 0xE7,       //   Usage Maximum (231)
    0x15, 0x00,       //   Logical Minimum (0)
    0x25, 0x01,       //   Logical Maximum (1)
    0x75, 0x01,       //   Report Size (1)
    0x95, 0x08,       //   Report Count (8)
    0x81, 0x02,       //   Input (Data, Variable, Absolute) - Modifier byte
    0x95, 0x01,       //   Report Count (1)
    0x75, 0x08,       //   Report Size (8)
    0x81, 0x01,       //   Input (Constant) - Reserved byte
    0x95, 0x06,       //   Report Count (6)
    0x75, 0x08,       //   Report Size (8)
    0x15, 0x00,       //   Logical Minimum (0)
    0x25, 0x65,       //   Logical Maximum (101)
    0x05, 0x07,       //   Usage Page (Key Codes)
    0x19, 0x00,       //   Usage Minimum (0)
    0x29, 0x65,       //   Usage Maximum (101)
    0x81, 0x00,       //   Input (Data, Array) - Key array
    0xC0              // End Collection
];
```

### 5. GUI Components

#### Credential List View

```rust
// src/simple_view.rs

pub fn create_credential_list(
    credentials: &[Credential],
    width: u32,
    height: u32
) -> Document {
    let mut document = Document::new(width, height);
    let mut menu = VerticalMenu::new(
        Rectangle::new(0, 0, width, height),
        &font::FONT_5X8
    );

    for cred in credentials {
        let label = format!("{} ({})", cred.name, cred.username);
        let item = VerticalMenuItem::new(&font::FONT_5X8, &label);
        menu.items_mut().push(item);
    }

    menu.set_on_activation(Box::new(|index| {
        // Show credential detail view
        Message::ShowCredentialDetail(index)
    }));

    document.components_mut().push(Box::new(menu));
    document.initialize_focus();
    document
}
```

#### Credential Detail View

```rust
// src/simple_gui/components/credential_detail.rs

pub struct CredentialDetail {
    credential: Credential,
    bounds: Rectangle,
    show_password: bool,
    font: &'static Font,
}

impl CredentialDetail {
    pub fn new(credential: Credential, bounds: Rectangle) -> Self {
        Self {
            credential,
            bounds,
            show_password: false,
            font: &font::FONT_5X8,
        }
    }
}

impl Component for CredentialDetail {
    fn draw(&self, bounds: Rectangle, commands: &mut Vec<RenderCommand>) {
        let mut y = self.bounds.y;

        // Name (large)
        let name_label = Label::new(self.font, &self.credential.name);
        name_label.set_position(Point::new(self.bounds.x, y));
        name_label.draw(bounds, commands);
        y += 10;

        // Username
        let username_label = Label::new(self.font, &format!("User: {}", self.credential.username));
        username_label.set_position(Point::new(self.bounds.x, y));
        username_label.draw(bounds, commands);
        y += 10;

        // Password (hidden or shown)
        let password_text = if self.show_password {
            self.credential.password.clone()
        } else {
            "••••••••".to_string()
        };
        let password_label = Label::new(self.font, &format!("Pass: {}", password_text));
        password_label.set_position(Point::new(self.bounds.x, y));
        password_label.draw(bounds, commands);
        y += 10;

        // Action hint
        let hint_label = Label::new(self.font, "↓ Type  ← Back");
        hint_label.set_position(Point::new(self.bounds.x, y));
        hint_label.draw(bounds, commands);
    }

    fn on_input(&mut self, events: &[InputEvent]) {
        for event in events {
            match (event.key_code, event.key_event) {
                (KeyCode::Middle, KeyEvent::Clicked) => {
                    // Toggle password visibility
                    self.show_password = !self.show_password;
                }
                (KeyCode::Down, KeyEvent::Clicked) => {
                    // Send message to type credentials
                    // TODO: Message passing system
                }
                _ => {}
            }
        }
    }
}
```

### 6. Web Vault Integration

#### New Angular Component

```typescript
// apps/web/src/app/tools/sync-to-device.component.ts

import { Component, OnInit } from '@angular/core';
import { CipherService } from '@bitwarden/common/vault/services/cipher.service';
import * as CBOR from 'cbor';

@Component({
  selector: 'app-sync-to-device',
  templateUrl: './sync-to-device.component.html',
})
export class SyncToDeviceComponent implements OnInit {
  cipherCount = 0;
  syncing = false;
  error: string | null = null;
  success = false;

  constructor(private cipherService: CipherService) {}

  async ngOnInit() {
    const ciphers = await this.cipherService.getAllDecrypted();
    this.cipherCount = ciphers.length;
  }

  async syncToDevice() {
    this.syncing = true;
    this.error = null;
    this.success = false;

    try {
      // Get all decrypted ciphers
      const ciphers = await this.cipherService.getAllDecrypted();

      // Convert to credential format
      const credentials = ciphers
        .filter(c => c.type === CipherType.Login)
        .map(c => ({
          id: c.id,
          name: c.name,
          username: c.login.username || '',
          password: c.login.password || '',
          uri: c.login.uris?.[0]?.uri || null,
          notes: c.notes || null,
        }));

      // Encode as CBOR
      const cborData = CBOR.encode({ credentials });

      // Send to device
      const response = await fetch('http://localhost:8080/api/sync', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/cbor',
        },
        body: cborData,
      });

      if (!response.ok) {
        throw new Error(`Sync failed: ${response.statusText}`);
      }

      this.success = true;
    } catch (e) {
      this.error = e.message || 'Failed to sync to device';
    } finally {
      this.syncing = false;
    }
  }
}
```

#### Template

```html
<!-- apps/web/src/app/tools/sync-to-device.component.html -->

<div class="page-header">
  <h1>Sync to Hardware Key</h1>
</div>

<div class="card">
  <div class="card-body">
    <p>Found {{ cipherCount }} credentials to sync.</p>

    <div *ngIf="error" class="alert alert-danger">
      {{ error }}
    </div>

    <div *ngIf="success" class="alert alert-success">
      Credentials synced successfully!
    </div>

    <button
      class="btn btn-primary"
      [disabled]="syncing"
      (click)="syncToDevice()"
    >
      <i class="bwi bwi-spinner bwi-spin" *ngIf="syncing"></i>
      {{ syncing ? 'Syncing...' : 'Sync to Device' }}
    </button>

    <div class="mt-3">
      <small class="text-muted">
        Make sure the desktop emulator or hardware key is connected and ready.
      </small>
    </div>
  </div>
</div>
```

## Implementation Phases

### Phase 1.1: Foundation (Week 1)
- ✅ Desktop emulator already working
- ✅ Focus system already working
- [ ] Add HTTP server to desktop emulator
- [ ] Implement credential data model
- [ ] Add CBOR encoding/decoding
- [ ] Test HTTP sync with curl/Postman

### Phase 1.2: Storage (Week 1-2)
- [ ] Desktop: Implement JSON file storage
- [ ] ESP32: Implement NVS storage module
- [ ] Test credential persistence
- [ ] Handle storage full scenarios

### Phase 1.3: GUI (Week 2)
- [ ] Credential list view (reuse VerticalMenu)
- [ ] Credential detail view component
- [ ] Navigation between views
- [ ] Show/hide password toggle

### Phase 1.4: Web Vault Integration (Week 2-3)
- [ ] Create sync-to-device Angular component
- [ ] Add CBOR encoding in TypeScript
- [ ] Test end-to-end desktop sync
- [ ] Error handling and user feedback

### Phase 1.5: Keyboard Output (Week 3)
- [ ] Desktop: Implement keyboard simulation (enigo crate)
- [ ] Test typing credentials into browser
- [ ] Handle special characters and timing

### Phase 1.6: ESP32 Port (Week 3-4)
- [ ] Port HTTP server to BLE service
- [ ] Implement BLE HID keyboard with esp32-nimble
- [ ] Configure NVS encryption
- [ ] Test on real hardware
- [ ] BLE pairing flow

### Phase 1.7: Polish (Week 4)
- [ ] Loading indicators
- [ ] Error messages
- [ ] Empty state handling
- [ ] Performance optimization
- [ ] Documentation

## Testing Strategy

### Unit Tests
- CBOR encoding/decoding
- Credential validation
- Storage operations
- GUI component rendering

### Integration Tests
- Desktop: HTTP sync flow
- ESP32: BLE sync flow
- Credential persistence
- Keyboard output

### Manual Testing
- Sync 10, 50, 100, 500 credentials
- Test scrolling performance
- Test on different browsers
- Test BLE pairing on iOS/Android/Windows/Mac
- Verify keyboard output on different sites

### Performance Targets
- Scroll latency: < 50ms
- Credential sync: < 5 seconds for 100 credentials
- Storage: Support 500+ credentials
- Keyboard typing: Natural speed (50-100ms between keys)

## Security Considerations

### Phase 1 (Current)
- Credentials stored in plaintext on device (NVS encryption only)
- HTTP in plaintext over localhost (desktop only)
- No authentication between Web Vault and device
- BLE pairing with 6-digit passkey

### Phase 2+ (Future)
- Encrypt credentials before storing
- Derive encryption key from user PIN
- Add device authentication
- Secure channel for BLE (bonding + encryption)
- Auto-lock timeout
- Secure memory clearing

## Open Questions

1. **Large vaults**: How to handle 1000+ credentials?
   - Pagination? Lazy loading? Search?

2. **Duplicate credentials**: Multiple logins for same site?
   - Show submenu? Numbered list?

3. **Special characters**: How to type non-ASCII passwords?
   - Unicode support? Fallback to copy/paste?

4. **Desktop keyboard simulation**: Which library?
   - `enigo`? `rdev`? `autopilot-rs`?

5. **BLE re-pairing**: How to handle bonding persistence?
   - Store LTK in NVS? Custom solution needed?

## References

- [2026-01-22-keyboard-emulation-first.md](decisions/2026-01-22-keyboard-emulation-first.md)
- [2026-01-22-emulator-http-protocol.md](decisions/2026-01-22-emulator-http-protocol.md)
- [2026-01-22-esp32-nvs-and-ble-hid.md](../.research/findings/2026-01-22-esp32-nvs-and-ble-hid.md)
- ESP32 BLE HID Reference: https://github.com/bjoernQ/esp32c3-ble-hid/
- CBOR RFC 8949: https://www.rfc-editor.org/rfc/rfc8949.html
