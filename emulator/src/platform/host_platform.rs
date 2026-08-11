//! `HostPlatform<D, I>`: the minimal wiring needed to instantiate a
//! `bhk_core::platform::Platform` on the host, generic over which
//! `DisplaySurface` (`D`) and `InputSource` (`I`) back it — the headless
//! and windowed run modes plug in `HeadlessSurface`/`NoopInput` or
//! `MinifbSurface`/`WindowedInput` respectively, sharing the same `Clock`
//! and `Storage` implementations either way.
//!
//! This exists to prove the capability-bundle trait actually assembles and
//! to give `emulator/examples/render_via_surfaces.rs` and
//! `emulator/tests/surface_parity.rs` something concrete to construct. It
//! is deliberately **not** the unified main loop (W7): nothing here polls
//! input, dispatches it to a `Navigator`, and re-renders in a loop. That
//! wiring — generic over `Platform` so it's shared by all three run modes
//! — is W7's job.

use bhk_core::platform::{DisplaySurface, InputSource, Platform};

use super::clock::HostClock;
use super::storage::FileStorage;

pub struct HostPlatform<D: DisplaySurface, I: InputSource> {
    display: D,
    input: I,
    clock: HostClock,
    storage: FileStorage,
}

impl<D: DisplaySurface, I: InputSource> HostPlatform<D, I> {
    #[must_use]
    pub fn new(display: D, input: I, storage: FileStorage) -> Self {
        Self { display, input, clock: HostClock::new(), storage }
    }
}

impl<D: DisplaySurface, I: InputSource> Platform for HostPlatform<D, I> {
    type Display = D;
    type Input = I;
    type Clock = HostClock;
    type Storage = FileStorage;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn input(&mut self) -> &mut Self::Input {
        &mut self.input
    }

    fn clock(&self) -> &Self::Clock {
        &self.clock
    }

    fn storage(&mut self) -> &mut Self::Storage {
        &mut self.storage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{HeadlessSurface, NoopInput};
    use bhk_core::platform::{Clock, Storage};
    use bhk_core::render::{FrameBuffer565, Navigator, Screen};

    #[test]
    fn a_headless_host_platform_can_be_assembled_and_used_through_the_platform_trait() {
        let storage = FileStorage::new(std::env::temp_dir().join(format!(
            "bhk-emulator-host-platform-test-{}.json",
            uuid::Uuid::new_v4()
        )))
        .unwrap();
        let mut platform = HostPlatform::new(HeadlessSurface::new(), NoopInput::new(), storage);

        // Exercise every accessor through the `Platform` trait object,
        // proving the bundle actually satisfies the trait, not just that
        // the struct compiles standalone.
        assert!(platform.input().poll().is_empty());
        let _now = platform.clock().now();
        assert_eq!(platform.storage().get("missing"), None);

        let navigator = Navigator::new(Screen::new("Test", vec![]));
        let mut framebuffer = FrameBuffer565::new(10, 10);
        navigator.render(&mut framebuffer).unwrap();
        platform.display().flush(&framebuffer).unwrap();
    }
}
