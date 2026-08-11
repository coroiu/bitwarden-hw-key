pub struct Timer {
    last_run: std::time::Instant,
    interval: std::time::Duration,
    looping: bool,
    running: bool,
}

impl Timer {
    pub fn new(interval: std::time::Duration, looping: bool) -> Self {
        Timer {
            last_run: std::time::Instant::now(),
            interval,
            looping,
            running: false,
        }
    }

    pub fn start(&mut self) {
        self.last_run = std::time::Instant::now();
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn run(&mut self) -> bool {
        if !self.running {
            return false;
        }

        let now = std::time::Instant::now();
        if now - self.last_run >= self.interval {
            self.last_run = now;
            self.running = self.looping;
            true
        } else {
            false
        }
    }
}
