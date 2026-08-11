// Library exports for the desktop emulator binary and its example tools
// (e.g. examples/json_to_cbor.rs, which needs the wire-format types below
// but can't depend on a `[[bin]]` target).

pub mod credentials;
pub mod desktop;
pub mod gui;
pub mod platform;
pub mod simple_gui;
pub mod simple_view;
