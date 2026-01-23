fn main() {
    let target = build_target::target();

    if target.arch == build_target::Arch::Xtensa {
        // ESP32 build setup
        embuild::espidf::sysenv::output();
    } else {
        // Desktop build - no special setup needed
    }
}
