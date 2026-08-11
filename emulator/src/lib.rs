// Library exports for the desktop emulator binary and its example tools
// (e.g. examples/json_to_cbor.rs, which needs the wire-format types below
// but can't depend on a `[[bin]]` target).
//
// The old `gui`/`simple_gui`/`simple_view` engines (128x32 mono, custom
// RGBA rasterizer) were retired in W7: `bhk_core::App` + `bhk_core::run`,
// driven through `platform`'s `HostPlatform`/surfaces/inputs, are their
// replacement — see `main.rs`.

pub mod credentials;
pub mod desktop;
pub mod platform;
