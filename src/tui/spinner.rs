pub struct Spinner {
    frames: Vec<&'static str>,
    current: usize,
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            current: 0,
        }
    }

    pub fn tick(&mut self) {
        self.current = (self.current + 1) % self.frames.len();
    }

    pub fn current(&self) -> &str {
        self.frames[self.current]
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}
