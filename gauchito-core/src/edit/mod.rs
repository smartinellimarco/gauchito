pub mod ot;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

impl Edit {
    pub fn new(start: usize, end: usize, text: String) -> Self {
        Self { start, end, text }
    }

    pub fn is_insert(&self) -> bool {
        self.start == self.end
    }

    pub fn is_delete(&self) -> bool {
        self.text.is_empty() && self.start != self.end
    }

    pub fn delta(&self) -> isize {
        self.text.len() as isize - (self.end - self.start) as isize
    }
}
