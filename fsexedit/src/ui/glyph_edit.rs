use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Paragraph, Widget};

use crate::app::App;
use crate::ui::preview::{Preview1x, Preview2x};

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
  let glyph = match app.current_glyph() {
    Some(g) => g,
    None => {
      Paragraph::new("No glyph loaded")
        .render(area, buf);
      return;
    }
  };

  // Split into title, main content, and status bar
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(1),  // title
      Constraint::Min(10),   // main
      Constraint::Length(2),  // status
    ])
    .split(area);

  // Title bar
  render_title(app, chunks[0], buf);

  // Main: grid on left, previews on right
  let main_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Length((glyph.pixels.width as u16 + 2) * 2 + 2), // grid + margins
      Constraint::Min(20), // previews
    ])
    .split(chunks[1]);

  render_grid(app, main_chunks[0], buf);
  render_previews(app, main_chunks[1], buf);

  // Status bar
  render_status(app, chunks[2], buf);
}

fn render_title(app: &App, area: Rect, buf: &mut Buffer) {
  let glyph = app.current_glyph().unwrap();
  let name = &glyph.name;
  let dirty = if app.font.dirty.contains(name) { " *" } else { "" };

  // Look up Unicode codepoint(s)
  let unicode_info = app.font.cmap_reverse.get(name)
    .map(|codes| {
      codes.iter()
        .map(|&c| {
          let ch = char::from_u32(c).map(|ch| format!(" \"{}\"", ch)).unwrap_or_default();
          let uname = unicode_names2::name(char::from_u32(c).unwrap_or('\0'))
            .map(|n| format!(" {}", n))
            .unwrap_or_default();
          format!("U+{:04X}{}{}", c, ch, uname)
        })
        .collect::<Vec<_>>()
        .join(", ")
    })
    .unwrap_or_else(|| "(no Unicode mapping)".to_string());

  let title = format!(" FSEX.ttx | {} | {}{}", name, unicode_info, dirty);
  buf.set_string(area.x, area.y, &title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
}

fn render_grid(app: &App, area: Rect, buf: &mut Buffer) {
  let glyph = app.current_glyph().unwrap();
  let grid = &glyph.pixels;

  let grid_w = grid.width;
  let grid_h = grid.height;
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

fn render_previews(app: &App, area: Rect, buf: &mut Buffer) {
  let glyph = app.current_glyph().unwrap();

  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(1),
      Constraint::Length(((glyph.pixels.height + 1) / 2) as u16 + 1),
      Constraint::Length(1),
      Constraint::Length(glyph.pixels.height as u16 + 1),
      Constraint::Min(0),
    ])
    .split(area);

  // 1x label
  buf.set_string(chunks[0].x, chunks[0].y, " 1x preview:", Style::default().fg(Color::DarkGray));

  // 1x preview
  let preview_area = Rect {
    x: chunks[1].x + 1,
    y: chunks[1].y,
    width: chunks[1].width.saturating_sub(1),
    height: chunks[1].height,
  };
  Preview1x { grid: &glyph.pixels }.render(preview_area, buf);

  // 2x label
  buf.set_string(chunks[2].x, chunks[2].y, " 2x preview:", Style::default().fg(Color::DarkGray));

  // 2x preview
  let preview_area = Rect {
    x: chunks[3].x + 1,
    y: chunks[3].y,
    width: chunks[3].width.saturating_sub(1),
    height: chunks[3].height,
  };
  Preview2x { grid: &glyph.pixels }.render(preview_area, buf);
}

fn render_status(app: &App, area: Rect, buf: &mut Buffer) {
  let keys = if app.floating.is_some() {
    " [←→↑↓]Position [R]otate [Enter]Commit [Esc]Cancel"
  } else {
    " [/]Search [Space]Toggle [←→↑↓]Move [n/p]Glyph [L]Ligatures [C]opy [V]Paste [M]ove [R]eset [E]xport [S]ave [Q]uit"
  };
  buf.set_string(area.x, area.y, keys, Style::default().fg(Color::DarkGray));

  // Status message
  if let Some(msg) = &app.status_message {
    if !msg.is_expired() {
      buf.set_string(area.x, area.y + 1, &msg.text, Style::default().fg(Color::Green));
    }
  }
}
