use std::collections::HashSet;

pub struct SequenceGenerator {
    current: u32,
    reserved: HashSet<u32>,
}

impl SequenceGenerator {
    pub fn new() -> Self {
        SequenceGenerator {
            current: 0,
            reserved: HashSet::new(),
        }
    }

    pub fn reserve(&mut self, value: u32) {
        self.reserved.insert(value);
    }

    pub fn next(&mut self) -> u32 {
        while self.reserved.contains(&self.current) {
            self.current += 1;
        }
        let next = self.current;
        self.current += 1;
        next
    }
}
