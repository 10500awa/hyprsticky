# hyprsticky

Tiny sticky notes for Hyprland, written in Rust. (vibecoded of course)

It uses about **4 MB** of ram just opened and about **50 MB** when a note has more than 1.5 Million characters. Each image in the open note adds a little.

- Every launch opens a new, empty note.
- Notes are plain markdown files in `~/.local/share/hyprsticky/`, saved automatically.
- Works with input methods (fcitx, ibus) and shows Chinese, Japanese and Korean text.
- Markdown is styled as you type: headings, bold, italic, strikethrough, code, links, quotes,
  lists, task lists and tables (monospace). The markers stay visible, dimmed.
- Copy, cut and paste. Paste a screenshot (or copy an image file) and it shows in the note.
- Six note colors: click the dot in the top right corner to change it.

## Install

```sh
git clone https://github.com/10500awa/hyprsticky
cd hyprsticky
cargo install --path .
```

Copy and paste need [wl-clipboard](https://github.com/bugaevc/wl-clipboard).

## Usage

```sh
hyprsticky              # new note
hyprsticky last         # reopen the last note
hyprsticky list         # saved notes
hyprsticky "Buy milk"   # reopen a note by name
```

`Esc` closes a note. Notes save a moment after you stop typing, and are renamed after their
first line when closed.

Keys: arrows, Home/End, Ctrl+arrows (by word), Ctrl+A, Ctrl+C/X/V, Ctrl+Z, Ctrl+Shift+Z.
Mouse: click, drag to select, double-click for a word, triple-click for a line.

Pasted images are saved in `attachments/` next to the notes and linked as
`![](attachments/…)`, so Obsidian shows them too. The color is saved in the note's front matter
(`color: pink`), which the note hides.

## Hyprland

Bind:
```lua
hl.bind(mod .. " + SHIFT + N", hl.dsp.exec_cmd("hyprsticky"))
```
Window rule:
```lua
hl.window_rule({
name = "hyprsticky",
float = true,
pin = true,
size = "260 260",
match = { class = "hyprsticky" }
})
``` 

Notes have no title bar, so move them with `SUPER` + drag. If a keybind can't find
`hyprsticky`, use the full path (`~/.cargo/bin/hyprsticky`).

## Settings

- `HYPRSTICKY_DIR`: where notes are stored. Point it at a folder in an Obsidian vault to manage
  them there.
- `HYPRSTICKY_FONT`: a fontconfig pattern (`monospace`) or a font file. Default: `sans-serif`.

## Limitations

Only PNG images are drawn (other image links stay text). Links are not opened.

## Todo

- Aur?

## License

See [LICENSE](LICENSE).
