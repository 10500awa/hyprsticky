//! A small TeX math typesetter: turns `\frac{a}{b}`, `x^2`, `\sqrt{x}`, `\sum_{i=1}^n`, Greek
//! letters, `\left( \right)` and matrices into positioned glyphs and rules. It draws nothing
//! and knows no fonts: the caller says how wide a character is, and draws the result.
//! Positions are in pixels, x from the left edge and y down from the baseline.

pub enum Item {
    Glyph {
        c: char,
        x: f32,
        y: f32,
        size: f32,
        italic: bool,
    },
    /// A filled rectangle: fraction bars, root bars, overlines.
    Rule {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

impl Item {
    fn shift(&mut self, dx: f32, dy: f32) {
        let (Item::Glyph { x, y, .. } | Item::Rule { x, y, .. }) = self;
        (*x, *y) = (*x + dx, *y + dy);
    }
}

/// A typeset piece: its items, width, and extent above (`asc`) and below (`desc`) the baseline.
#[derive(Default)]
pub struct Bx {
    pub items: Vec<Item>,
    pub w: f32,
    pub asc: f32,
    pub desc: f32,
}

impl Bx {
    fn space(w: f32) -> Bx {
        Bx { w, ..Default::default() }
    }

    /// Adds `b` with its origin at (dx, dy).
    fn put(&mut self, mut b: Bx, dx: f32, dy: f32) {
        if b.items.is_empty() && b.w == 0.0 {
            return;
        }
        b.items.iter_mut().for_each(|it| it.shift(dx, dy));
        self.items.append(&mut b.items);
        (self.w, self.asc, self.desc) =
            (self.w.max(dx + b.w), self.asc.max(b.asc - dy), self.desc.max(b.desc + dy));
    }

    /// Adds `b` after the current content.
    fn push(&mut self, b: Bx) {
        let x = self.w;
        self.put(b, x, 0.0);
    }
}

/// Height of the math axis (the middle of `+` and `=`, and of fraction bars) in em.
const AXIS: f32 = 0.27;

/// Name and symbol pairs, separated by spaces.
const SYMBOLS: &str = "alpha α beta β gamma γ delta δ epsilon ε varepsilon ε zeta ζ eta η \
    theta θ vartheta ϑ iota ι kappa κ lambda λ mu μ nu ν xi ξ pi π rho ρ sigma σ tau τ upsilon υ \
    phi φ varphi φ chi χ psi ψ omega ω Gamma Γ Delta Δ Theta Θ Lambda Λ Xi Ξ Pi Π Sigma Σ \
    Upsilon Υ Phi Φ Psi Ψ Omega Ω cdot ⋅ times × div ÷ pm ± mp ∓ ast ∗ circ ∘ bullet • leq ≤ \
    le ≤ geq ≥ ge ≥ neq ≠ ne ≠ approx ≈ equiv ≡ sim ∼ cong ≅ propto ∝ ll ≪ gg ≫ in ∈ notin ∉ \
    subset ⊂ subseteq ⊆ supset ⊃ supseteq ⊇ cup ∪ cap ∩ emptyset ∅ varnothing ∅ forall ∀ \
    exists ∃ neg ¬ land ∧ lor ∨ to → rightarrow → leftarrow ← leftrightarrow ↔ Rightarrow ⇒ \
    Leftarrow ⇐ Leftrightarrow ⇔ implies ⟹ iff ⟺ mapsto ↦ infty ∞ partial ∂ nabla ∇ ldots … \
    dots … cdots ⋯ vdots ⋮ ddots ⋱ prime ′ angle ∠ perp ⊥ parallel ∥ langle ⟨ rangle ⟩ \
    lfloor ⌊ rfloor ⌋ lceil ⌈ rceil ⌉ degree ° ell ℓ hbar ℏ Re ℜ Im ℑ aleph ℵ star ⋆ \
    therefore ∴ because ∵ sum ∑ prod ∏ int ∫ oint ∮ iint ∬ bigcup ⋃ bigcap ⋂";

/// Functions written upright, like `\sin`.
const FUNCTIONS: &str = "sin cos tan cot sec csc arcsin arccos arctan sinh cosh tanh log ln lg \
    exp lim max min sup inf det gcd arg deg dim ker mod Pr";

fn symbol(name: &str) -> Option<char> {
    let mut it = SYMBOLS.split(' ');
    while let (Some(n), Some(c)) = (it.next(), it.next()) {
        if n == name {
            return c.chars().next();
        }
    }
    None
}

/// Rough (height above, depth below) the baseline of a character, in em.
fn extent(c: char) -> (f32, f32) {
    match c {
        'a' | 'c' | 'e' | 'm' | 'n' | 'o' | 'r' | 's' | 'u' | 'v' | 'w' | 'x' | 'z' => (0.56, 0.0),
        'g' | 'j' | 'p' | 'q' | 'y' | 'α'..='ω' => (0.56, 0.21),
        '(' | ')' | '[' | ']' | '{' | '}' | '|' | '∑' | '∏' | '∫' | ',' | ';' => (0.76, 0.21),
        _ => (0.74, 0.0),
    }
}

struct P<'a> {
    s: Vec<char>,
    i: usize,
    /// The body font size, which scripts never shrink below half of.
    em: f32,
    /// Width of a character at a size, upright or italic.
    adv: &'a mut dyn FnMut(char, f32, bool) -> f32,
}

/// Typesets `tex` at `em` pixels per em. `adv(c, size, italic)` gives a character's width.
pub fn typeset(tex: &str, em: f32, adv: &mut dyn FnMut(char, f32, bool) -> f32) -> Bx {
    let mut p = P { s: tex.chars().collect(), i: 0, em, adv };
    let mut out = Bx::default();
    while p.i < p.s.len() {
        out.push(p.row(em));
        // Skip what ended the row (a stray `}`, `&`...), which also guarantees progress.
        p.i += if p.starts("\\\\") { 2 } else { 1 };
    }
    out
}

impl P<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn starts(&self, t: &str) -> bool {
        t.chars().enumerate().all(|(k, c)| self.s.get(self.i + k) == Some(&c))
    }

    /// Is the next token the command `\name` (and not a longer one)?
    fn at_cmd(&self, name: &str) -> bool {
        self.starts(&format!("\\{name}"))
            && !self.s.get(self.i + 1 + name.len()).is_some_and(|c| c.is_alphabetic())
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    /// Does the current row end here?
    fn at_end(&self) -> bool {
        matches!(self.peek(), None | Some('}' | '&'))
            || self.starts("\\\\")
            || self.at_cmd("end")
            || self.at_cmd("right")
    }

    /// A command name: a run of letters, or one other character.
    fn name(&mut self) -> String {
        let start = self.i;
        while self.peek().is_some_and(char::is_alphabetic) {
            self.i += 1;
        }
        if self.i == start && self.peek().is_some() {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }

    /// The text of a `{group}` (or one character), unparsed.
    fn raw(&mut self) -> String {
        self.skip_space();
        if self.peek() != Some('{') {
            let c = self.peek();
            self.i += 1;
            return c.map(String::from).unwrap_or_default();
        }
        let (start, mut depth) = (self.i + 1, 0);
        while let Some(c) = self.peek() {
            self.i += 1;
            depth += (c == '{') as i32 - (c == '}') as i32;
            if depth == 0 {
                break;
            }
        }
        self.s[start..self.i.saturating_sub(1).max(start)].iter().collect()
    }

    fn glyph(&mut self, c: char, size: f32, italic: bool) -> Bx {
        let c = match c {
            '-' => '−',
            '\'' => '′',
            _ => c,
        };
        let (asc, desc) = extent(c);
        let items = vec![Item::Glyph { c, x: 0.0, y: 0.0, size, italic }];
        Bx { items, w: (self.adv)(c, size, italic), asc: asc * size, desc: desc * size }
    }

    /// Upright text.
    fn text(&mut self, s: &str, size: f32) -> Bx {
        let mut b = Bx::default();
        for c in s.chars() {
            b.push(if c == ' ' { Bx::space(0.3 * size) } else { self.glyph(c, size, false) });
        }
        b
    }

    /// Typesets a string with the same fonts.
    fn sub(&mut self, tex: &str, size: f32) -> Bx {
        P { s: tex.chars().collect(), i: 0, em: self.em, adv: &mut *self.adv }.row(size)
    }

    /// Things in a row, until its end (`}`, `&`, `\\`, `\end` or `\right`).
    fn row(&mut self, size: f32) -> Bx {
        let mut out = Bx::default();
        let mut operand = false; // is there something before, so that `-` is a binary operator?
        loop {
            self.skip_space();
            if self.at_end() {
                return out;
            }
            let b = self.atom(size);
            let c = if let [Item::Glyph { c, .. }] = b.items[..] { c } else { '\0' };
            let bin = "+−×⋅±∓∗÷∘".contains(c);
            let rel = "=<>≤≥≠≈≡∼≅∝≪≫→←↔⇒⇐⇔⟹⟺↦∈∉⊂⊆⊃⊇".contains(c);
            let gap = size
                * if rel {
                    0.28
                } else if bin && operand {
                    0.22
                } else {
                    0.0
                };
            out.push(Bx::space(gap));
            out.push(b);
            out.push(Bx::space(if c == ',' { 0.17 * size } else { gap }));
            operand = !(bin || rel);
        }
    }

    /// One thing, with its scripts.
    fn atom(&mut self, size: f32) -> Bx {
        let (base, limits) = self.base(size);
        self.scripts(base, limits, size)
    }

    /// An argument: a `{group}`, a command or one character, without scripts.
    fn arg(&mut self, size: f32) -> Bx {
        self.skip_space();
        if self.at_end() { Bx::default() } else { self.base(size).0 }
    }

    /// Returns the box and whether scripts go above and below it (`\sum`, `\lim`).
    fn base(&mut self, size: f32) -> (Bx, bool) {
        let c = self.s[self.i];
        self.i += 1;
        match c {
            '{' => {
                let b = self.row(size);
                if self.peek() == Some('}') {
                    self.i += 1;
                }
                (b, false)
            }
            '\\' => self.command(size),
            '^' | '_' => {
                self.i -= 1; // `scripts` takes it, as in `{}^2`
                (Bx::default(), false)
            }
            '~' => (Bx::space(0.3 * size), false),
            _ => (self.glyph(c, size, c.is_ascii_alphabetic()), false),
        }
    }

    fn scripts(&mut self, base: Bx, limits: bool, size: f32) -> Bx {
        let small = (size * 0.7).max(self.em * 0.5);
        let (mut sup, mut sub) = (Bx::default(), Bx::default());
        loop {
            self.skip_space();
            match self.peek() {
                Some(m @ ('^' | '_')) => {
                    self.i += 1;
                    let arg = self.arg(small);
                    *(if m == '^' { &mut sup } else { &mut sub }) = arg;
                }
                _ => break,
            }
        }
        if sup.items.is_empty() && sub.items.is_empty() {
            return base;
        }
        let (ba, bd, bw) = (base.asc, base.desc, base.w);
        let (sw, uw, sa, sd) = (sup.w, sub.w, sub.asc, sup.desc);
        let mut b = Bx::default();
        if limits {
            let (w, gap) = (bw.max(sw).max(uw), 0.12 * size);
            b.put(base, (w - bw) / 2.0, 0.0);
            b.put(sup, (w - sw) / 2.0, -(ba + gap + sd));
            b.put(sub, (w - uw) / 2.0, bd + gap + sa);
        } else {
            let up = (ba - 0.3 * small).max(0.4 * size);
            let mut down = (bd + 0.1 * size).max(0.2 * size);
            // Keep both scripts apart when there are two.
            down += (0.15 * size - (up + down - sa - sd)).max(0.0);
            let x = bw + 0.03 * size;
            b.put(base, 0.0, 0.0);
            b.put(sup, x, -up);
            b.put(sub, x, down);
        }
        b
    }

    fn command(&mut self, size: f32) -> (Bx, bool) {
        let name = self.name();
        let limits =
            matches!(name.as_str(), "sum" | "prod" | "bigcup" | "bigcap" | "lim" | "max" | "min");
        let b = match name.as_str() {
            "frac" | "dfrac" | "tfrac" => self.frac(size),
            "sqrt" => self.sqrt(size),
            "left" => {
                let l = self.delimiter();
                let inner = self.row(size);
                let mut r = '.';
                if self.at_cmd("right") {
                    self.i += 6;
                    r = self.delimiter();
                }
                self.fenced(inner, l, r, size)
            }
            "begin" => self.env(size),
            "text" | "textrm" | "mathrm" | "operatorname" | "mbox" => {
                let t = self.raw();
                self.text(&t, size)
            }
            "overline" | "bar" => {
                let c = self.arg(size);
                let (th, y) = ((size / 16.0).max(1.0), 0.1 * size);
                let mut b = Bx { w: c.w, ..Default::default() };
                b.items.push(Item::Rule { x: 0.0, y: -c.asc - y - th, w: c.w, h: th });
                b.asc = c.asc + y + th;
                b.put(c, 0.0, 0.0);
                b
            }
            "hat" | "tilde" | "vec" => {
                let c = self.arg(size);
                let (mark, drop) = match name.as_str() {
                    "hat" => ('^', 0.25),
                    "tilde" => ('~', 0.2),
                    _ => ('→', 0.06),
                };
                let a = self.glyph(mark, 0.8 * size, false);
                let mut b = Bx::default();
                let dx = (c.w - a.w) / 2.0;
                b.put(a, dx, -c.asc + drop * size);
                b.put(c, 0.0, 0.0);
                b
            }
            "mathbf" | "mathbb" | "mathcal" | "mathit" | "mathsf" | "boldsymbol" | "bm" => {
                self.arg(size)
            }
            "displaystyle" | "textstyle" | "limits" | "nolimits" | "big" | "Big" | "bigg" | "!" => {
                Bx::default()
            }
            "," => Bx::space(0.17 * size),
            ";" | ":" | " " => Bx::space(0.28 * size),
            "quad" => Bx::space(size),
            "qquad" => Bx::space(2.0 * size),
            "{" | "}" | "%" | "$" | "#" | "&" | "_" => {
                self.glyph(name.chars().next().unwrap(), size, false)
            }
            "|" => self.glyph('‖', size, false),
            "sum" | "prod" | "int" | "oint" | "iint" | "bigcup" | "bigcap" => {
                // Big operators are drawn larger and centred on the axis.
                let (s, mut b) = (1.4 * size, Bx::default());
                let g = self.glyph(symbol(&name).unwrap_or('?'), s, false);
                b.put(g, 0.0, 0.3 * s - AXIS * size);
                b
            }
            n if FUNCTIONS.split(' ').any(|f| f == n) => {
                let mut b = self.text(n, size);
                b.push(Bx::space(if limits { 0.0 } else { 0.17 * size }));
                b
            }
            n => match symbol(n) {
                Some(c) => self.glyph(c, size, ('α'..='ω').contains(&c)),
                None => self.text(n, size),
            },
        };
        (b, limits)
    }

    fn frac(&mut self, size: f32) -> Bx {
        let fs = (size * 0.85).max(self.em * 0.5);
        let (n, d) = (self.arg(fs), self.arg(fs));
        let (th, axis, gap) = ((size / 16.0).max(1.0), AXIS * size, 0.15 * size);
        let w = n.w.max(d.w) + 0.2 * size;
        let (nw, dw, nd, da) = (n.w, d.w, n.desc, d.asc);
        let mut b = Bx { w, ..Default::default() };
        b.put(n, (w - nw) / 2.0, -(axis + th / 2.0 + gap + nd));
        b.put(d, (w - dw) / 2.0, -axis + th / 2.0 + gap + da);
        b.items.push(Item::Rule { x: 0.0, y: -axis - th / 2.0, w, h: th });
        b
    }

    fn sqrt(&mut self, size: f32) -> Bx {
        let mut index = Bx::default();
        if self.peek() == Some('[') {
            let end = self.s[self.i..]
                .iter()
                .position(|&c| c == ']')
                .map_or(self.s.len(), |p| self.i + p);
            let tex: String = self.s[self.i + 1..end].iter().collect();
            self.i = (end + 1).min(self.s.len());
            index = self.sub(&tex, 0.5 * size);
        }
        let c = self.arg(size);
        let (th, gap) = ((size / 16.0).max(1.0), 0.12 * size);
        let top = c.asc + gap + th;
        // The root sign's ink runs from 0.02 below the baseline to 0.81 above it.
        let s = (top + c.desc) / 0.83;
        let rad = self.glyph('√', s, false);
        let (dx, rw, cw, cd) = (index.w, rad.w, c.w, c.desc);
        let (start, end) = (dx + rw, dx + rw + cw + 0.1 * size);
        let mut b = Bx::default();
        b.put(index, 0.0, -0.55 * top);
        b.put(rad, dx, cd - 0.02 * s);
        b.put(c, start + 0.05 * size, 0.0);
        b.items.push(Item::Rule { x: start - 0.5, y: -top, w: end - start + 0.5, h: th });
        (b.w, b.asc) = (b.w.max(end), b.asc.max(top));
        b
    }

    /// A delimiter after `\left` or `\right`; `.` means none.
    fn delimiter(&mut self) -> char {
        self.skip_space();
        match self.peek() {
            Some('\\') => {
                self.i += 1;
                match self.name().as_str() {
                    "{" => '{',
                    "}" => '}',
                    "|" => '‖',
                    n => symbol(n).unwrap_or('.'),
                }
            }
            Some(c) => {
                self.i += 1;
                c
            }
            None => '.',
        }
    }

    /// `inner` between delimiters stretched to its height.
    fn fenced(&mut self, inner: Bx, l: char, r: char, size: f32) -> Bx {
        let (asc, desc) = (inner.asc, inner.desc);
        let mut b = Bx::default();
        b.push(self.delim(l, asc, desc, size));
        b.push(inner);
        b.push(self.delim(r, asc, desc, size));
        b
    }

    /// A delimiter covering `asc` above and `desc` below the baseline (never below one line).
    fn delim(&mut self, c: char, asc: f32, desc: f32, size: f32) -> Bx {
        if c == '.' {
            return Bx::default();
        }
        let axis = AXIS * size;
        let half = (asc - axis).max(desc + axis);
        // Brackets have ink from -0.13 to 0.76 of their size: centre that on the axis.
        let s = (2.0 * half / 0.89).max(size);
        let mut b = Bx::default();
        let g = self.glyph(c, s, false);
        b.put(g, 0.0, 0.3 * s - axis);
        b
    }

    /// `\begin{name} cells \end{name}`: a grid of cells separated by `&` and `\\`.
    fn env(&mut self, size: f32) -> Bx {
        let name = self.raw();
        if name == "array" {
            self.raw(); // the column layout
        }
        let mut rows: Vec<Vec<Bx>> = Vec::new();
        loop {
            let mut cells = Vec::new();
            loop {
                cells.push(self.row(size));
                if self.peek() != Some('&') {
                    break;
                }
                self.i += 1;
            }
            rows.push(cells);
            if !self.starts("\\\\") {
                break;
            }
            self.i += 2;
        }
        if self.at_cmd("end") {
            self.i += 4;
            self.raw();
        }
        if rows.last().is_some_and(|r| r.iter().all(|c| c.items.is_empty())) {
            rows.pop(); // a trailing `\\`
        }
        let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        let width =
            |j: usize| rows.iter().filter_map(|r| r.get(j)).map(|c| c.w).fold(0.0, f32::max);
        let widths: Vec<f32> = (0..cols).map(width).collect();
        let heights: Vec<(f32, f32)> = rows
            .iter()
            .map(|r| {
                let max = |f: fn(&Bx) -> f32| r.iter().map(f).fold(0.0, f32::max);
                (max(|c| c.asc).max(0.7 * size), max(|c| c.desc).max(0.2 * size))
            })
            .collect();
        let gap = 0.2 * size;
        let total = heights.iter().map(|h| h.0 + h.1 + gap).sum::<f32>() - gap;
        let (mut top, mut grid) = (-AXIS * size - total / 2.0, Bx::default());
        for (row, (asc, desc)) in rows.into_iter().zip(heights) {
            let (base, mut x) = (top + asc, 0.0);
            for (cell, w) in row.into_iter().zip(&widths) {
                let pad = if name == "cases" { 0.0 } else { (w - cell.w) / 2.0 };
                grid.put(cell, x + pad, base);
                x += w + size;
            }
            top = base + desc + gap;
        }
        let (l, r) = match name.as_str() {
            "pmatrix" => ('(', ')'),
            "bmatrix" => ('[', ']'),
            "Bmatrix" => ('{', '}'),
            "vmatrix" => ('|', '|'),
            "Vmatrix" => ('‖', '‖'),
            "cases" => ('{', '.'),
            _ => ('.', '.'),
        };
        self.fenced(grid, l, r, size)
    }
}
