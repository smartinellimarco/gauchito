use ropey::Rope;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splice {
    p: usize,
    q: usize,
    text: String,
}

impl Splice {
    pub fn new(p: usize, q: usize, text: String) -> Self {
        assert!(p <= q, "Splice::new: p ({p}) must be <= q ({q})");
        Self { p, q, text }
    }

    pub fn p(&self) -> usize {
        self.p
    }

    pub fn q(&self) -> usize {
        self.q
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_identity(&self) -> bool {
        self.p == self.q && self.text.is_empty()
    }

    pub fn apply(&self, rope: &mut Rope) -> Splice {
        let removed = if self.q > self.p {
            let s = rope.slice(self.p..self.q).to_string();

            rope.remove(self.p..self.q);

            s
        } else {
            String::new()
        };

        if !self.text.is_empty() {
            rope.insert(self.p, &self.text);
        }

        Splice {
            p: self.p,
            q: self.p + self.text.chars().count(),
            text: removed,
        }
    }
}

pub fn phi(s: &Splice, r: usize) -> usize {
    let n = s.text.chars().count();
    let o = s.q - s.p;

    if r < s.p {
        r
    } else if r <= s.q {
        s.p + n
    } else if n >= o {
        r + (n - o)
    } else {
        r - (o - n)
    }
}
