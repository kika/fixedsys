use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

use crate::font::PixelGrid;

/// 1x preview using half-block characters (▀▄█ ).
/// Each character cell encodes two vertical pixels.
pub struct Preview1x<'a> {
  pub grid: &'a PixelGrid,
}

impl Widget for Preview1x<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let rows_needed = (self.grid.height + 1) / 2;
    for row_pair in 0..rows_needed {
      if row_pair as u16 >= area.height {
        break;
      }
      for col in 0..self.grid.width {
        if col as u16 >= area.width {
          break;
        }
        let top = self.grid.get(col, row_pair * 2);
        let bottom = self.grid.get(col, row_pair * 2 + 1);
        let ch = match (top, bottom) {
          (true, true) => '█',
          (true, false) => '▀',
          (false, true) => '▄',
          (false, false) => ' ',
        };
        buf.set_string(
          area.x + col as u16,
          area.y + row_pair as u16,
          &ch.to_string(),
          Style::default().fg(Color::White),
        );
      }
    }
  }
}

/// 2x preview: each pixel = 2 chars wide, 1 row tall (using ██ / ░░).
pub struct Preview2x<'a> {
  pub grid: &'a PixelGrid,
}

impl Widget for Preview2x<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    for row in 0..self.grid.height {
      if row as u16 >= area.height {
        break;
      }
      for col in 0..self.grid.width {
        let x = col as u16 * 2;
        if x + 1 >= area.width {
          break;
        }
        let filled = self.grid.get(col, row);
        let s = if filled { "██" } else { "░░" };
        buf.set_string(
          area.x + x,
          area.y + row as u16,
          s,
          Style::default().fg(if filled { Color::White } else { Color::DarkGray }),
        );
      }
    }
  }
}
