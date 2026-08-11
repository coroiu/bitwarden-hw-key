# Desktop Emulator HTTP Protocol for BLE Simulation

**Date**: 2026-01-22
**Status**: Superseded (2026-08-11), demoted to development/fallback mechanism

> **2026-08-11 update:** This HTTP + CBOR push protocol is no longer the product sync
> path. The 2026-08-11 vision session set the direction that the device is a
> first-class Bitwarden client that authenticates, syncs, and decrypts on-device via
> the Bitwarden Rust SDK (see `../roadmap.md`). This local HTTP push (the Web Vault or
> a companion pushing already-decrypted credentials to the emulator) is retained only
> as a development aid and as the fallback if the on-device SDK feasibility spike shows
> direct server sync is impractical on the ESP32-S3 for now. The endpoint and CBOR
> design below still accurately describe that dev/fallback path.

## Context

Desktop emulation enables rapid development without flashing hardware. To test the credential sync flow from Web Vault → Device, we need a way to simulate BLE communication. Two approaches considered:

1. **Mock BLE Server** - Separate service that bridges Web Vault ↔ Desktop Emulator
2. **Direct HTTP Server** - Desktop emulator runs HTTP server, Web Vault connects directly

## Decision

The **desktop emulator will run an embedded HTTP server** that the Web Vault Angular app connects to directly. No separate mock BLE server needed.

## Rationale

### Why Direct HTTP Server

1. **Simplicity**: One less component to run/manage
2. **Lower latency**: Direct connection, no proxy hop
3. **Easier debugging**: All logs in one place
4. **Matches real architecture**: ESP32 will receive data directly over BLE, emulator mirrors this
5. **Standard HTTP**: No WebSocket complexity needed for this use case

### Why Not Mock BLE Server

1. **Unnecessary complexity**: Adds another process to manage
2. **Harder to debug**: Split logs across multiple components
3. **Extra latency**: Proxy hop adds overhead
4. **Deployment friction**: Need to start/stop separate service

## HTTP Protocol Design

### Architecture

```
┌─────────────────────────────────────┐
│  Web Vault (Angular)                │
│  localhost:4200                     │
│  - User auth                        │
│  - CBOR encoding                    │
│  - HTTP client                      │
└────────────┬────────────────────────┘
             │ HTTP POST
             │ /api/sync
             │ Content-Type: application/cbor
             ▼
┌─────────────────────────────────────┐
│  Desktop Emulator (Rust)            │
│  localhost:8080                     │
│  - HTTP server                      │
│  - CBOR decoding                    │
│  - NVS simulation                   │
│  - GUI rendering                    │
└─────────────────────────────────────┘
```

### Endpoints

#### `POST /api/sync`
Sync credentials from Web Vault to device.

**Request Headers**:
```
Content-Type: application/cbor
```

**Request Body** (CBOR-encoded):
```rust
struct SyncRequest {
    credentials: Vec<Credential>,
}

struct Credential {
    id: String,           // UUID
    name: String,         // "GitHub"
    username: String,     // "user@example.com"
    password: String,     // "secret123"
    uri: Option<String>,  // "https://github.com"
    notes: Option<String>,
}
```

**Response** (200 OK):
```json
{
  "status": "success",
  "synced": 42,
  "total_bytes": 12345
}
```

**Response** (400 Bad Request):
```json
{
  "status": "error",
  "message": "Invalid CBOR encoding"
}
```

#### `GET /api/status`
Check if emulator is ready to receive credentials.

**Response** (200 OK):
```json
{
  "ready": true,
  "stored_credentials": 42,
  "storage_used_bytes": 12345,
  "storage_capacity_bytes": 65536
}
```

#### `POST /api/clear`
Clear all stored credentials (for testing).

**Response** (200 OK):
```json
{
  "status": "success",
  "cleared": 42
}
```

### CORS Configuration

Since Web Vault runs on `localhost:4200` and emulator on `localhost:8080`, need CORS headers:

```
Access-Control-Allow-Origin: http://localhost:4200
Access-Control-Allow-Methods: POST, GET, OPTIONS
Access-Control-Allow-Headers: Content-Type
```

### Error Handling

- **Connection refused**: Emulator not running → Show clear error in Web Vault UI
- **Decode error**: Invalid CBOR → Return 400 with descriptive message
- **Storage full**: Not enough space → Return 507 (Insufficient Storage)

## Implementation Plan

### Desktop Emulator Changes

**Dependencies**:
```toml
[target.'cfg(not(target_arch = "xtensa"))'.dependencies]
minifb = "0.27"
tiny_http = "0.12"  # Lightweight HTTP server
ciborium = "0.2"    # CBOR encoding/decoding
```

**HTTP Server Module** (`src/desktop/http_server.rs`):
```rust
pub struct SyncServer {
    server: tiny_http::Server,
    credentials: Arc<Mutex<Vec<Credential>>>,
}

impl SyncServer {
    pub fn new() -> Result<Self>;
    pub fn handle_request(&self) -> Result<()>;
    pub fn get_credentials(&self) -> Vec<Credential>;
}
```

**Integration in Main Loop**:
```rust
// src/bin/desktop.rs
fn main() {
    let server = SyncServer::new().unwrap();
    let credentials = server.credentials.clone();

    std::thread::spawn(move || {
        loop {
            server.handle_request().ok();
        }
    });

    // Existing main loop
    while window.is_open() {
        // Check for new credentials
        let creds = credentials.lock().unwrap();
        if !creds.is_empty() {
            document.update_credentials(&creds);
        }
        // ... rest of loop
    }
}
```

### Web Vault Changes

**New Component**: `sync-to-device.component.ts` in Web Vault Angular app

**Service** (`device-sync.service.ts`):
```typescript
export class DeviceSyncService {
  private readonly EMULATOR_URL = 'http://localhost:8080';

  async syncCredentials(credentials: Credential[]): Promise<void> {
    const cbor = CBOR.encode({ credentials });

    const response = await fetch(`${this.EMULATOR_URL}/api/sync`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/cbor' },
      body: cbor,
    });

    if (!response.ok) {
      throw new Error(`Sync failed: ${response.statusText}`);
    }
  }

  async checkStatus(): Promise<DeviceStatus> {
    const response = await fetch(`${this.EMULATOR_URL}/api/status`);
    return response.json();
  }
}
```

## ESP32 Transition

When moving to real hardware, the HTTP server code is replaced with BLE service:

```rust
// HTTP endpoint        → BLE Characteristic
POST /api/sync         → Write to credential_sync_char UUID
GET /api/status        → Read from device_status_char UUID
```

Same CBOR encoding/decoding logic is reused. Only transport layer changes.

## Testing Strategy

1. **Unit tests**: Test CBOR encoding/decoding with sample credentials
2. **Integration test**: Run emulator + Web Vault, verify sync works
3. **Error cases**: Test connection refused, invalid CBOR, storage full
4. **Performance**: Test sync speed with 100, 500, 1000 credentials

## Alternatives Considered

### Alternative 1: WebSocket Connection
- **Pros**: Bidirectional, can push updates to Web Vault
- **Cons**: More complex, overkill for one-way sync, harder to debug

### Alternative 2: Mock BLE Server
- **Pros**: More realistic BLE simulation
- **Cons**: Extra component, more complex, harder debugging

### Alternative 3: File-based Sync
- **Pros**: Simplest, no network
- **Cons**: Doesn't test any networking code, unrealistic

## Consequences

### Positive
- Simple architecture with minimal moving parts
- Easy to debug with standard HTTP tools (curl, Postman)
- CORS enables Web Vault integration
- Smooth transition to BLE on ESP32 (same CBOR protocol)

### Negative
- Requires running emulator before opening Web Vault sync page
- HTTP server adds dependency to desktop build
- Not testing actual BLE stack (but that's intentional for emulator)

### Mitigations
- Web Vault shows clear error if emulator not running
- HTTP server is tiny (~50KB) with `tiny_http`
- BLE testing will happen on real hardware later

## References

- CBOR RFC: https://www.rfc-editor.org/rfc/rfc8949.html
- `tiny_http` docs: https://docs.rs/tiny_http/
- `ciborium` docs: https://docs.rs/ciborium/
