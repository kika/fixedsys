# fsexedit — Fixedsys TTX TUI Editor

A terminal UI editor for the Fixedsys Excelsior font's TTX source file. Renders glyphs as editable pixel grids directly in the terminal — no GUI font editor needed.

## Build

Requires Rust toolchain (1.70+).

```bash
cargo build --release
```

## Usage

```bash
cargo run --release -- ../FSEX.ttx
```

Or after building:

```bash
./target/release/fsexedit ../FSEX.ttx
```

### Diagnostic flags

```bash
fsexedit FSEX.ttx --dump        # Print font stats and glyph "A" as ASCII art, then exit
fsexedit FSEX.ttx --save-test   # Save unmodified font to FSEX.ttx.roundtrip, then exit
fsexedit FSEX.ttx --edit-test   # Toggle pixel (0,0) on "A", save to FSEX.ttx.edited, then exit
```

## Keyboard Controls

### Glyph Edit Mode

| Key | Action |
|-----|--------|
| Arrow keys | Move cursor on pixel grid |
| Space | Toggle pixel under cursor |
| N / P | Next / previous glyph |
| / | Search glyphs |
| L | Search ligatures |
| S | Save |
| Q | Quit |

### Ligature Edit Mode

| Key | Action |
|-----|--------|
| Arrow keys | Move cursor on pixel grid |
| Space | Toggle pixel under cursor |
| N / P | Next / previous ligature |
| / | Search ligatures |
| G | Switch to glyph search |
| S | Save |
| Q | Quit |

### Search Overlay

| Key | Action |
|-----|--------|
| Type text | Filter results |
| Up / Down | Navigate results |
| Enter | Select result |
| Esc | Cancel |

Glyph search accepts: `U+0041`, `A`, `arrow` (glyph name substring), or `CAPITAL LETTER` (Unicode name substring).

Ligature search accepts: `->`, `===` (trigger text), or `rightarrow` (result glyph name).

## Layout

The editor shows an editable pixel grid on the left with margin guides, and 1x/2x previews on the right. The title bar displays the glyph name, Unicode codepoint, and character name. A `*` indicates unsaved changes.

```
 FSEX.ttx | A | U+0041 "A" LATIN CAPITAL LETTER A
······················    1x preview:
··░░░░░░████░░░░····     ▀█▀
··░░░░████████░░····     █ █
··░░████░░░░████····     ███
··░░████░░░░████····     █ █
··░░████░░░░████····     █ █
··░░████████████····
··░░████░░░░████····    2x preview:
··░░████░░░░████····    ░░░░░░████░░░░░░
··░░████░░░░████····    ░░░░████████░░░░
······················  ░░████░░░░████░░
 [/]Search [Space]Toggle [←→↑↓]Move [n/p]Glyph [L]Ligatures [S]Save [Q]Quit
```
