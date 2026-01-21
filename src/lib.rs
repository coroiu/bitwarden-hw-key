// Library exports for desktop emulation and shared code

pub mod gui;
pub mod simple_gui;
pub mod simple_view;

// Desktop emulation support (only compiled for non-ESP32 targets)
#[cfg(not(target_arch = "xtensa"))]
pub mod desktop;
