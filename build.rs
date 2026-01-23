#[cfg(target_arch = "xtensa")]
fn main() {
    // Only run ESP-IDF build setup for xtensa targets (ESP32)
    // #[cfg(target_arch = "xtensa")]
    embuild::espidf::sysenv::output();
}

#[cfg(not(target_arch = "xtensa"))]
fn main() {
    // No build setup needed for non-xtensa targets
}
