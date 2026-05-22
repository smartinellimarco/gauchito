use ropey::Rope;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    p: usize,
    q: usize,
    t: String,
}

impl Atom {
    pub fn new(p: usize, q: usize, t: String) -> Self {
        assert!(p <= q, "Atom::new: p ({p}) must be <= q ({q})");
        Self { p, q, t }
    }

    pub fn p(&self) -> usize {
        self.p
    }

    pub fn q(&self) -> usize {
        self.q
    }

    pub fn t(&self) -> &str {
        &self.t
    }

    pub fn is_identity(&self) -> bool {
        self.p == self.q && self.t.is_empty()
    }

    pub fn apply(&self, rope: &mut Rope) -> Atom {
        let removed = if self.q > self.p {
            let s = rope.slice(self.p..self.q).to_string();

            rope.remove(self.p..self.q);

            s
        } else {
            String::new()
        };

        if !self.t.is_empty() {
            rope.insert(self.p, &self.t);
        }

        Atom {
            p: self.p,
            q: self.p + self.t.chars().count(),
            t: removed,
        }
    }
}

pub fn phi(m: &Atom, r: usize) -> usize {
    let n = m.t.chars().count();
    let o = m.q - m.p;

    if r < m.p {
        r
    } else if r <= m.q {
        m.p + n
    } else if n >= o {
        r + (n - o)
    } else {
        r - (o - n)
    }
}
