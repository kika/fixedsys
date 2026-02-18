use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::app::App;
use crate::ui::preview::{Preview1x, Preview2x};

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
  let lig = match app.current_ligature() {
    Some(l) => l,
    None => return,
  };

  let glyph = match app.font.glyphs.get(&lig.result_glyph) {
    Some(g) => g,
    None => return,
  };

  let trigger = lig.trigger_text(&app.font.cmap_reverse);

  // Split: title, main, status
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(1),
      Constraint::Min(10),
      Constraint::Length(2),
    ])
    .split(area);

  // Title
  let dirty = if app.font.dirty.contains(&lig.result_glyph) { " *" } else { "" };
  let title = format!(
    " Ligature: \"{}\" → {} ({}){}", trigger, lig.result_glyph,
    format!("{} + {}", lig.first_glyph, lig.components.join(" + ")),
    dirty,
  );
  buf.set_string(chunks[0].x, chunks[0].y, &title,
    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD));

  // Main: grid + previews
  let grid = &glyph.pixels;
  let main_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Length((grid.width as u16 + 2) * 2 + 2),
      Constraint::Min(20),
    ])
    .split(chunks[1]);

  // Grid with cell dividers
  render_ligature_grid(app, glyph, main_chunks[0], buf);

  // Previews
  let preview_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(1),
      Constraint::Length(((grid.height + 1) / 2) as u16 + 1),
      Constraint::Length(1),
      Constraint::Length(grid.height as u16 + 1),
      Constraint::Min(0),
    ])
    .split(main_chunks[1]);

  buf.set_string(preview_chunks[0].x, preview_chunks[0].y, " 1x preview:",
    Style::default().fg(Color::DarkGray));
  let pa = Rect {
    x: preview_chunks[1].x + 1, y: preview_chunks[1].y,
    width: preview_chunks[1].width.saturating_sub(1), height: preview_chunks[1].height,
  };
  Preview1x { grid }.render(pa, buf);

  buf.set_string(preview_chunks[2].x, preview_chunks[2].y, " 2x preview:",
    Style::default().fg(Color::DarkGray));
  let pa = Rect {
    x: preview_chunks[3].x + 1, y: preview_chunks[3].y,
    width: preview_chunks[3].width.saturating_sub(1), height: preview_chunks[3].height,
  };
  Preview2x { grid }.render(pa, buf);

  // Status bar
  let keys = if app.floating.is_some() {
    " [←→↑↓]Position [R]otate [Enter]Commit [Esc]Cancel"
  } else {
    " [/]Search [Space]Toggle [←→↑↓]Move [G]Glyph mode [C]opy [V]Paste [M]ove [R]eset [E]xport [S]ave [Q]uit"
  };
  buf.set_string(chunks[2].x, chunks[2].y, keys, Style::default().fg(Color::DarkGray));

  if let Some(msg) = &app.status_message {
    if !msg.is_expired() {
      buf.set_string(chunks[2].x, chunks[2].y + 1, &msg.text,
        Style::default().fg(Color::Green));
    }
  }
}

fn render_ligature_grid(
  app: &App,
  glyph: &crate::font::Glyph,
  area: Rect,
  buf: &mut Buffer,
) {
  let grid = &glyph.pixels;
  let grid_w = grid.width;
  let grid_h = grid.height;
  let cell_width = 8; // Each character cell is 8 pixels wide
  let has_floating = app.floating.is_some();

  for row in 0..grid_h + 2 {
    for col in 0..grid_w + 2 {
      let x = area.x + col as u16 * 2;
      let y = area.y + row as u16;

      if y >= area.y + area.height || x + 1 >= area.x + area.width {
        continue;
      }

      let is_margin = row == 0 || row == grid_h + 1 || col == 0 || col == grid_w + 1;

      if is_margin {
        buf.set_string(x, y, "··", Style::default().fg(Color::DarkGray));
      } else {
        let pixel_col = col - 1;
        let pixel_row = row - 1;
        let filled = grid.get(pixel_col, pixel_row);

        let is_cell_divider = (col - 1) % cell_width == 0 && (col - 1) > 0;

        // Check if floating pixel covers this position
        let floating_here = if let Some(fl) = &app.floating {
          let fr = pixel_row as i32 - fl.offset_row;
          let fc = pixel_col as i32 - fl.offset_col;
          fr >= 0 && fr < fl.pixels.height as i32
            && fc >= 0 && fc < fl.pixels.width as i32
            && fl.pixels.get(fc as usize, fr as usize)
        } else {
          false
        };

        let is_cursor = !has_floating
          && pixel_col == app.cursor_col
          && pixel_row == app.cursor_row;

        let (s, style) = if floating_here {
          ("██", Style::default().fg(Color::Magenta))
        } else if is_cursor {
          if filled {
            ("██", Style::default().fg(Color::Yellow).bg(Color::Blue))
          } else {
            ("░░", Style::default().fg(Color::Yellow).bg(Color::Blue))
          }
        } else if is_cell_divider && !filled {
          ("┆┆", Style::default().fg(Color::Rgb(60, 40, 40)))
        } else if filled {
          ("██", Style::default().fg(Color::White))
        } else {
          ("░░", Style::default().fg(Color::Rgb(40, 40, 40)))
        };
        buf.set_string(x, y, s, style);
      }
    }
  }
}
