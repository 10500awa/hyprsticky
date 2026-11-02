//! Clipboard through wl-clipboard (`wl-copy` and `wl-paste`). `wl-copy` keeps serving the text
//! after we exit, which a window that closes with Esc could not do by itself.

use crate::store;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Puts text on the clipboard.
pub fn copy(text: &str) {
    if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        let _ = child.stdin.take().map(|mut stdin| stdin.write_all(text.as_bytes()));
        let _ = child.wait();
    }
}

/// What to insert for Ctrl+V: the clipboard text, or for an image (or image files) the markdown
/// that shows a copy stored in the attachments folder. Empty if there is nothing to paste.
pub fn paste(dir: &Path) -> String {
    let get = |args: &[&str]| {
        Command::new("wl-paste").args(args).output().map(|out| out.stdout).unwrap_or_default()
    };
    let text = |args: &[&str]| String::from_utf8_lossy(&get(args)).into_owned();
    let types = text(&["-l"]);
    if types.lines().any(|t| t == "text/uri-list") {
        let links: Vec<_> = text(&["-t", "text/uri-list"])
            .lines()
            .filter_map(|uri| attach_file(dir, uri))
            .collect();
        if !links.is_empty() {
            return links.join("\n");
        }
    }
    if types.lines().any(|t| t.starts_with("text/plain") || t == "UTF8_STRING") {
        return text(&["-n", "-t", "text"]);
    }
    let image = types.lines().filter(|t| t.starts_with("image/")).max_by_key(|t| *t == "image/png");
    image.and_then(|t| store::attach(dir, &t[6..], &get(&["-t", t]))).unwrap_or_default()
}

/// Copies the image file a `file://` address points to into the attachments folder.
fn attach_file(dir: &Path, uri: &str) -> Option<String> {
    let path = PathBuf::from(unescape(uri.strip_prefix("file://")?));
    let ext = path.extension()?.to_str()?.to_lowercase();
    if !["png", "jpg", "jpeg", "gif", "webp", "bmp"].contains(&ext.as_str()) {
        return None;
    }
    store::attach(dir, &ext, &fs::read(&path).ok()?)
}

/// Decodes `%XX` escapes.
fn unescape(s: &str) -> String {
    let (b, mut out, mut i) = (s.as_bytes(), Vec::new(), 0);
    while i < b.len() {
        let hex = b
            .get(i + 1..i + 3)
            .and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
        match (b[i], hex) {
            (b'%', Some(v)) => {
                out.push(v);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
