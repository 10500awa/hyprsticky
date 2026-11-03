//! Inline images. A PNG is decoded the first time a note shows it, scaled down to the note's
//! width and kept as a small bitmap, so memory only grows with the images in the note (about
//! width x height x 4 bytes each). The full-size pixels are held in an anonymous mapping that
//! goes back to the system as soon as the scaling is done. Formulas are drawn the same way.

use crate::{
    font::Fonts,
    math::{self, Item},
};
use memmap2::MmapMut;
use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
};

/// Larger decoded images are not shown (their link still is), to keep the peak small.
const MAX_DECODED: usize = 64 << 20;

/// An image scaled to fit the note: premultiplied BGRA pixels.
pub struct Image {
    pub w: usize,
    pub h: usize,
    /// The row of the text baseline, for a formula that sits in a line of text.
    pub base: usize,
    pub bgra: Vec<u8>,
}

pub struct Images {
    dir: PathBuf,
    width: u32,
    cache: HashMap<String, Option<Image>>,
    /// The previous layout's images, until this layout has claimed the ones it still shows.
    old: HashMap<String, Option<Image>>,
}

impl Images {
    /// `dir` is the notes folder, which image paths in notes are relative to.
    pub fn new(dir: &Path) -> Images {
        Images { dir: dir.to_path_buf(), width: 0, cache: HashMap::new(), old: HashMap::new() }
    }

    /// Call before laying a note out, and `sweep` after: images the note no longer shows are
    /// dropped (a formula being typed is a new image on every key).
    pub fn start(&mut self) {
        self.old = std::mem::take(&mut self.cache);
    }

    pub fn sweep(&mut self) {
        self.old = HashMap::new();
    }

    /// The image at `src`, at most `width` pixels wide; made on first use. A `src` that starts
    /// with `$` is a centred display formula (TeX), one that starts with `\u{1}` a formula as
    /// wide as it is, for a line of text, anything else a PNG file: None if it is missing or not
    /// a PNG. Changing `width` drops every cached image.
    pub fn get(&mut self, src: &str, width: u32, fonts: &mut Fonts) -> Option<&Image> {
        if width != self.width {
            (self.width, self.cache, self.old) = (width, HashMap::new(), HashMap::new());
        }
        let path = self.dir.join(src.replace("%20", " "));
        let old = self.old.remove(src);
        let image = self.cache.entry(src.to_string()).or_insert_with(|| {
            old.unwrap_or_else(|| match src.chars().next() {
                Some(c @ ('$' | '\u{1}')) => {
                    formula(&src[1..], width as usize, fonts, c == '\u{1}')
                }
                _ => load(&path, width as usize),
            })
        });
        image.as_ref()
    }

    /// An image that `get` has loaded.
    pub fn cached(&self, src: &str) -> Option<&Image> {
        self.cache.get(src)?.as_ref()
    }
}

/// Draws a formula in black; a formula wider than `width` pixels is shrunk to fit. An `inline`
/// one is as wide as it is and as large as the text; a display one is a little larger and
/// centred in `width`.
fn formula(tex: &str, width: usize, fonts: &mut Fonts, inline: bool) -> Option<Image> {
    let em = fonts.px() * if inline { 1.0 } else { 1.2 };
    let mut set = |em: f32| {
        math::typeset(tex, em, &mut |c, size, italic| {
            fonts.advance_px(c, (italic as u8) << 1, size)
        })
    };
    let mut m = set(em);
    let room = (width as f32 - if inline { 2.0 } else { 0.0 }).max(1.0);
    if m.w > room {
        m = set(em * room / m.w);
    }
    let h = ((m.asc + m.desc).ceil() as usize + 2).min(4096);
    if m.items.is_empty() || width == 0 {
        return None;
    }
    let (x0, y0) = (if inline { 1.0 } else { ((width as f32 - m.w) / 2.0).max(0.0) }, m.asc + 1.0);
    let width = if inline { m.w.ceil() as usize + 2 } else { width };
    let mut alpha = vec![0u8; width * h];
    let mut dot = |x: i32, y: i32, a: u8| {
        if (0..width as i32).contains(&x) && (0..h as i32).contains(&y) {
            let p = &mut alpha[y as usize * width + x as usize];
            *p = p.saturating_add(a);
        }
    };
    for item in &m.items {
        match *item {
            Item::Glyph { c, x, y, size, italic } => {
                let Some(g) = fonts.raster(c, (italic as u8) << 1, size) else { continue };
                let (gx, gy) = ((x0 + x).round() as i32 + g.left, (y0 + y).round() as i32 + g.top);
                for (k, &a) in g.cov.iter().enumerate() {
                    dot(gx + (k as u32 % g.w) as i32, gy + (k as u32 / g.w) as i32, a);
                }
            }
            Item::Rule { x, y, w, h: thick } => {
                let (xa, ya) = ((x0 + x).round() as i32, (y0 + y).round() as i32);
                let xb = ((x0 + x + w).round() as i32).max(xa + 1);
                for py in ya..ya + (thick.round() as i32).max(1) {
                    (xa..xb).for_each(|px| dot(px, py, 255));
                }
            }
        }
    }
    let bgra = alpha.iter().flat_map(|&a| [0, 0, 0, a]).collect();
    Some(Image { w: width, h, base: y0.round() as usize, bgra })
}

fn load(path: &Path, width: usize) -> Option<Image> {
    let mut decoder = png::Decoder::new(File::open(path).ok()?);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let len = reader.output_buffer_size();
    if len > MAX_DECODED {
        return None;
    }
    let mut pixels = MmapMut::map_anon(len).ok()?;
    let info = reader.next_frame(&mut pixels).ok()?;
    let size = (info.width as usize, info.height as usize);
    Some(scale(&pixels, size, info.line_size, info.color_type.samples(), width))
}

/// Averages `buf` (`size` pixels of `ch` samples: gray, gray and alpha, RGB or RGBA) down to
/// fit `width`, and to at most 3 times that in height. Never scales up.
fn scale(buf: &[u8], (sw, sh): (usize, usize), stride: usize, ch: usize, width: usize) -> Image {
    let f = (width as f32 / sw as f32).min(3.0 * width as f32 / sh as f32).min(1.0);
    let (w, h) = (((sw as f32 * f) as usize).max(1), ((sh as f32 * f) as usize).max(1));
    let mut bgra = Vec::with_capacity(w * h * 4);
    for dy in 0..h {
        let (y0, y1) = (dy * sh / h, (dy + 1) * sh / h);
        for dx in 0..w {
            let (x0, x1) = (dx * sw / w, (dx + 1) * sw / w);
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = &buf[y * stride + x * ch..][..ch];
                    let (r, g, b, a) = match ch {
                        1 => (p[0], p[0], p[0], 255),
                        2 => (p[0], p[0], p[0], p[1]),
                        3 => (p[0], p[1], p[2], 255),
                        _ => (p[0], p[1], p[2], p[3]),
                    };
                    for (s, c) in sum.iter_mut().zip([b, g, r]) {
                        *s += c as u32 * a as u32 / 255;
                    }
                    sum[3] += a as u32;
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            bgra.extend(sum.map(|s| (s / n) as u8));
        }
    }
    Image { w, h, base: 0, bgra }
}
