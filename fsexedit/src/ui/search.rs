use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::app::{App, SearchTarget};

/// Render the search overlay as a centered popup.
pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
  let popup_w = 60u16.min(area.width.saturating_sub(4));
  let popup_h = 20u16.min(area.height.saturating_sub(4));
  let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
  let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 2;

  // Clear popup area
  for y in popup_y..popup_y + popup_h {
    for x in popup_x..popup_x + popup_w {
      buf.set_string(x, y, " ", Style::default().bg(Color::Rgb(30, 30, 40)));
    }
  }

  // Border (simple box)
  let border_style = Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 40));
  for x in popup_x..popup_x + popup_w {
    buf.set_string(x, popup_y, "─", border_style);
    buf.set_string(x, popup_y + popup_h - 1, "─", border_style);
  }
  for y in popup_y..popup_y + popup_h {
    buf.set_string(popup_x, y, "│", border_style);
    buf.set_string(popup_x + popup_w - 1, y, "│", border_style);
  }
  buf.set_string(popup_x, popup_y, "┌", border_style);
  buf.set_string(popup_x + popup_w - 1, popup_y, "┐", border_style);
  buf.set_string(popup_x, popup_y + popup_h - 1, "└", border_style);
  buf.set_string(popup_x + popup_w - 1, popup_y + popup_h - 1, "┘", border_style);

  let inner_x = popup_x + 2;
  let inner_w = popup_w.saturating_sub(4);

  // Title
  let title = match app.search_target {
    SearchTarget::Glyph => "Search Glyph",
    SearchTarget::Ligature => "Search Ligature",
  };
  buf.set_string(inner_x, popup_y, &format!(" {} ", title),
    Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 40)).add_modifier(Modifier::BOLD));

  // Input field
  let input_y = popup_y + 2;
  let bg = Style::default().bg(Color::Rgb(30, 30, 40)).fg(Color::White);
  buf.set_string(inner_x, input_y, "> ", bg);
  let input_display = if app.search_input.len() as u16 > inner_w - 2 {
    &app.search_input[app.search_input.len() - (inner_w as usize - 2)..]
  } else {
    &app.search_input
  };
  buf.set_string(inner_x + 2, input_y, input_display, bg.add_modifier(Modifier::BOLD));
  // Cursor
  let cursor_x = inner_x + 2 + input_display.len() as u16;
  if cursor_x < popup_x + popup_w - 2 {
    buf.set_string(cursor_x, input_y, "█",
      Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 40)));
  }

  // Help text
  let help_y = popup_y + 3;
  let help = match app.search_target {
    SearchTarget::Glyph => "U+hex, char, glyph name, or Unicode name",
    SearchTarget::Ligature => "trigger text (->), glyph name, or U+hex",
  };
  buf.set_string(inner_x, help_y, help,
    Style::default().fg(Color::DarkGray).bg(Color::Rgb(30, 30, 40)));

  // Results
  let results_y = popup_y + 5;
  let max_results = (popup_h as usize).saturating_sub(7);

  if app.search_results.is_empty() && !app.search_input.is_empty() {
    buf.set_string(inner_x, results_y, "No matches found",
      Style::default().fg(Color::Red).bg(Color::Rgb(30, 30, 40)));
  } else {
    for (i, result) in app.search_results.iter().take(max_results).enumerate() {
      let y = results_y + i as u16;
      if y >= popup_y + popup_h - 1 {
        break;
      }
      let is_selected = i == app.search_selected;
      let is_create = result.starts_with("+ Create");
      let style = if is_selected {
        Style::default().fg(Color::Black).bg(Color::Cyan)
      } else if is_create {
        Style::default().fg(Color::Green).bg(Color::Rgb(30, 30, 40))
      } else {
        Style::default().fg(Color::White).bg(Color::Rgb(30, 30, 40))
      };
      // Truncate result to fit
      let display: String = result.chars().take(inner_w as usize).collect();
      // Clear the line first
      let blank: String = " ".repeat(inner_w as usize);
      buf.set_string(inner_x, y, &blank, style);
      buf.set_string(inner_x, y, &display, style);
    }
  }

  // Bottom hint
  buf.set_string(inner_x, popup_y + popup_h - 2, "[Enter]Select [Esc]Cancel [↑↓]Navigate",
    Style::default().fg(Color::DarkGray).bg(Color::Rgb(30, 30, 40)));
}
