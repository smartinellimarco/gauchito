use serde::{Deserialize, Serialize};

use crate::ot::transform_pos;
use crate::Edit;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn new(pos: usize) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn range(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    pub fn ordered(&self) -> (usize, usize) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn is_forward(&self) -> bool {
        self.anchor <= self.head
    }

    pub fn transform(&self, edit: &Edit) -> Self {
        Self {
            anchor: transform_pos(self.anchor, edit),
            head: transform_pos(self.head, edit),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionGroup {
    pub sels: Vec<Selection>,
    pub primary: usize,
}

impl SelectionGroup {
    pub fn single(pos: usize) -> Self {
        Self {
            sels: vec![Selection::new(pos)],
            primary: 0,
        }
    }

    pub fn primary(&self) -> &Selection {
        &self.sels[self.primary]
    }

    pub fn primary_mut(&mut self) -> &mut Selection {
        &mut self.sels[self.primary]
    }

    pub fn transform(&self, edit: &Edit) -> Self {
        Self {
            sels: self.sels.iter().map(|s| s.transform(edit)).collect(),
            primary: self.primary,
        }
    }
}

impl Default for SelectionGroup {
    fn default() -> Self {
        Self::single(0)
    }
}
