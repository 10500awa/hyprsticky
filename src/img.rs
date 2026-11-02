//! Inline images. A PNG is decoded the first time a note shows it, scaled down to the note's
//! width and kept as a small bitmap, so memory only grows with the images in the note (about
//! width x height x 4 bytes each). The full-size pixels are held in an anonymous mapping that
//! goes back to the system as soon as the scaling is done.

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
    pub bgra: Vec<u8>,
}

pub struct Images {
    dir: PathBuf,
    width: u32,
    cache: HashMap<String, Option<Image>>,
}

impl Images {
    /// `dir` is the notes folder, which image paths in notes are relative to.
    pub fn new(dir: &Path) -> Images {
        Images { dir: dir.to_path_buf(), width: 0, cache: HashMap::new() }
    }

    /// The image at `src`, at most `width` pixels wide; loaded on first use. None if the file
    /// is missing or not a PNG. Changing `width` drops every cached image.
    pub fn get(&mut self, src: &str, width: u32) -> Option<&Image> {
        if width != self.width {
            (self.width, self.cache) = (width, HashMap::new());
        }
        let path = self.dir.join(src.replace("%20", " "));
        let image =
            self.cache.entry(src.to_string()).or_insert_with(|| load(&path, width as usize));
        image.as_ref()
    }

    /// An image that `get` has loaded.
    pub fn cached(&self, src: &str) -> Option<&Image> {
        self.cache.get(src)?.as_ref()
    }
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
    Image { w, h, bgra }
}
