mod app;
mod font;
mod grid;
mod ttx;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
  disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, Mode, SearchTarget};

fn main() -> Result<()> {
  let args: Vec<String> = std::env::args().collect();
  if args.len() < 2 {
    eprintln!("Usage: fsexedit <path-to-ttx>");
    eprintln!("  e.g. fsexedit FSEX.ttx");
    std::process::exit(1);
  }
  let path = &args[1];

  eprintln!("Loading {}...", path);
  let mut font = ttx::parse::parse_ttx(path).context("Failed to parse TTX file")?;
  eprintln!(
    "Loaded {} glyphs, {} Unicode mappings, {} ligatures",
    font.glyphs.len(),
    font.cmap.len(),
    font.ligatures.len()
  );

  // If --dump flag, print stats and exit (for testing)
  if args.iter().any(|a| a == "--dump") {
    dump_stats(&font);
    return Ok(());
  }

  // If --save-test flag, save to a temp file and exit
  if args.iter().any(|a| a == "--save-test") {
    let out_path = format!("{}.roundtrip", path);
    ttx::write::save_ttx(&mut font, &out_path)?;
    eprintln!("Saved to {}", out_path);
    return Ok(());
  }

  // If --edit-test flag, toggle a pixel on "A" and save
  if args.iter().any(|a| a == "--edit-test") {
    let out_path = format!("{}.edited", path);
    if let Some(glyph) = font.glyphs.get_mut("A") {
      glyph.pixels.toggle(0, 0); // Toggle top-left pixel
      font.dirty.insert("A".to_string());
    }
    ttx::write::save_ttx(&mut font, &out_path)?;
    eprintln!("Saved edited font to {}", out_path);
    return Ok(());
  }

  // Set up terminal
  enable_raw_mode().context("enable raw mode")?;
  let mut stdout = io::stdout();
  execute!(stdout, EnterAlternateScreen).context("enter alt screen")?;
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend).context("create terminal")?;

  let mut app = App::new(font, path.to_string());

  let result = run_event_loop(&mut terminal, &mut app);

  // Restore terminal
  disable_raw_mode().ok();
  execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
  terminal.show_cursor().ok();

  result
}

fn run_event_loop(
  terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
  app: &mut App,
) -> Result<()> {
  loop {
    terminal.draw(|f| draw(f, app))?;

    if app.should_quit {
      break;
    }

    if event::poll(Duration::from_millis(100))? {
      if let Event::Key(key) = event::read()? {
        if key.kind != KeyEventKind::Press {
          continue;
        }
        handle_key(app, key.code, key.modifiers);
      }
    }
  }
  Ok(())
}

fn draw(f: &mut ratatui::Frame, app: &App) {
  let area = f.area();

  match app.mode {
    Mode::GlyphEdit => {
      ui::glyph_edit::render(app, area, f.buffer_mut());
    }
    Mode::LigatureEdit => {
      ui::ligature_edit::render(app, area, f.buffer_mut());
    }
    Mode::Search => {
      // Draw underlying mode first, then overlay
      match app.prev_mode {
        Mode::GlyphEdit | Mode::Search | Mode::Confirm => {
          ui::glyph_edit::render(app, area, f.buffer_mut());
        }
        Mode::LigatureEdit => {
          ui::ligature_edit::render(app, area, f.buffer_mut());
        }
      }
      ui::search::render(app, area, f.buffer_mut());
    }
    Mode::Confirm => {
      // Draw underlying mode, then confirm dialog
      ui::glyph_edit::render(app, area, f.buffer_mut());
      draw_confirm(app, area, f.buffer_mut());
    }
  }
}

fn draw_confirm(_app: &App, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
  use ratatui::style::{Color, Modifier, Style};

  let w = 48u16.min(area.width.saturating_sub(4));
  let h = 7u16;
  let x = area.x + (area.width.saturating_sub(w)) / 2;
  let y = area.y + (area.height.saturating_sub(h)) / 2;

  let bg = Style::default().bg(Color::Rgb(40, 20, 20)).fg(Color::White);
  let border_style = Style::default().fg(Color::Red).bg(Color::Rgb(40, 20, 20));

  // Fill background
  for dy in 0..h {
    for dx in 0..w {
      buf.set_string(x + dx, y + dy, " ", bg);
    }
  }

  // Border
  for dx in 1..w - 1 {
    buf.set_string(x + dx, y, "─", border_style);
    buf.set_string(x + dx, y + h - 1, "─", border_style);
  }
  for dy in 1..h - 1 {
    buf.set_string(x, y + dy, "│", border_style);
    buf.set_string(x + w - 1, y + dy, "│", border_style);
  }
  buf.set_string(x, y, "┌", border_style);
  buf.set_string(x + w - 1, y, "┐", border_style);
  buf.set_string(x, y + h - 1, "└", border_style);
  buf.set_string(x + w - 1, y + h - 1, "┘", border_style);

  // Title
  buf.set_string(x + 2, y, " Unsaved changes! ",
    bg.add_modifier(Modifier::BOLD).fg(Color::Red));

  // Options
  buf.set_string(x + 2, y + 3, "[S]", bg.add_modifier(Modifier::BOLD).fg(Color::Yellow));
  buf.set_string(x + 5, y + 3, "ave", bg);
  buf.set_string(x + 10, y + 3, "[Q]", bg.add_modifier(Modifier::BOLD).fg(Color::Yellow));
  buf.set_string(x + 13, y + 3, "uit without saving", bg);
  buf.set_string(x + 33, y + 3, "[Esc]", bg.add_modifier(Modifier::BOLD).fg(Color::Yellow));
  buf.set_string(x + 38, y + 3, "Cancel", bg);
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
  match app.mode {
    Mode::GlyphEdit => handle_glyph_edit_key(app, code, modifiers),
    Mode::LigatureEdit => handle_ligature_edit_key(app, code, modifiers),
    Mode::Search => handle_search_key(app, code),
    Mode::Confirm => handle_confirm_key(app, code),
  }
}

fn handle_glyph_edit_key(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
  if app.floating.is_some() {
    handle_floating_key(app, code);
    return;
  }
  match code {
    KeyCode::Char('q') | KeyCode::Char('Q') => {
      if !app.font.dirty.is_empty() {
        app.mode = Mode::Confirm;
      } else {
        app.should_quit = true;
      }
    }
    KeyCode::Char('s') | KeyCode::Char('S') => app.save(),
    KeyCode::Char(' ') => app.toggle_pixel(),
    KeyCode::Up => app.move_cursor(-1, 0),
    KeyCode::Down => app.move_cursor(1, 0),
    KeyCode::Left => app.move_cursor(0, -1),
    KeyCode::Right => app.move_cursor(0, 1),
    KeyCode::Char('n') | KeyCode::Char('N') => app.next_glyph(),
    KeyCode::Char('p') | KeyCode::Char('P') => app.prev_glyph(),
    KeyCode::Char('/') => app.open_search(SearchTarget::Glyph),
    KeyCode::Char('l') | KeyCode::Char('L') => app.open_search(SearchTarget::Ligature),
    KeyCode::Char('c') | KeyCode::Char('C') => app.copy(),
    KeyCode::Char('v') | KeyCode::Char('V') => app.start_paste(),
    KeyCode::Char('m') | KeyCode::Char('M') => app.start_move(),
    KeyCode::Char('r') | KeyCode::Char('R') => app.reset_canvas(),
    KeyCode::Char('e') | KeyCode::Char('E') => app.export_svg(),
    _ => {}
  }
}

fn handle_ligature_edit_key(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
  if app.floating.is_some() {
    handle_floating_key(app, code);
    return;
  }
  match code {
    KeyCode::Char('q') | KeyCode::Char('Q') => {
      if !app.font.dirty.is_empty() {
        app.mode = Mode::Confirm;
      } else {
        app.should_quit = true;
      }
    }
    KeyCode::Char('s') | KeyCode::Char('S') => app.save(),
    KeyCode::Char(' ') => app.toggle_pixel(),
    KeyCode::Up => app.move_cursor(-1, 0),
    KeyCode::Down => app.move_cursor(1, 0),
    KeyCode::Left => app.move_cursor(0, -1),
    KeyCode::Right => app.move_cursor(0, 1),
    KeyCode::Char('n') | KeyCode::Char('N') => app.next_ligature(),
    KeyCode::Char('p') | KeyCode::Char('P') => app.prev_ligature(),
    KeyCode::Char('/') => app.open_search(SearchTarget::Ligature),
    KeyCode::Char('g') | KeyCode::Char('G') => app.open_search(SearchTarget::Glyph),
    KeyCode::Char('c') | KeyCode::Char('C') => app.copy(),
    KeyCode::Char('v') | KeyCode::Char('V') => app.start_paste(),
    KeyCode::Char('m') | KeyCode::Char('M') => app.start_move(),
    KeyCode::Char('r') | KeyCode::Char('R') => app.reset_canvas(),
    KeyCode::Char('e') | KeyCode::Char('E') => app.export_svg(),
    _ => {}
  }
}

fn handle_floating_key(app: &mut App, code: KeyCode) {
  match code {
    KeyCode::Up => app.move_floating(-1, 0),
    KeyCode::Down => app.move_floating(1, 0),
    KeyCode::Left => app.move_floating(0, -1),
    KeyCode::Right => app.move_floating(0, 1),
    KeyCode::Char('r') | KeyCode::Char('R') => app.rotate_floating(),
    KeyCode::Enter => app.commit_floating(),
    KeyCode::Esc => app.cancel_floating(),
    _ => {}
  }
}

fn handle_search_key(app: &mut App, code: KeyCode) {
  match code {
    KeyCode::Esc => app.close_search(),
    KeyCode::Enter => app.select_search_result(),
    KeyCode::Backspace => {
      app.search_input.pop();
      app.update_search();
    }
    KeyCode::Char(c) => {
      app.search_input.push(c);
      app.update_search();
    }
    KeyCode::Up => {
      if app.search_selected > 0 {
        app.search_selected -= 1;
      }
    }
    KeyCode::Down => {
      if app.search_selected + 1 < app.search_results.len() {
        app.search_selected += 1;
      }
    }
    _ => {}
  }
}

fn handle_confirm_key(app: &mut App, code: KeyCode) {
  match code {
    KeyCode::Char('s') | KeyCode::Char('S') => {
      app.save();
      app.should_quit = true;
    }
    KeyCode::Char('q') | KeyCode::Char('Q') => {
      app.should_quit = true;
    }
    KeyCode::Esc => {
      app.mode = Mode::GlyphEdit;
    }
    _ => {}
  }
}

fn dump_stats(font: &font::Font) {
  println!("Glyphs: {}", font.glyphs.len());
  println!("Glyph order: {}", font.glyph_order.len());
  println!("Unicode mappings (cmap): {}", font.cmap.len());
  println!("Ligatures: {}", font.ligatures.len());
  println!("Units per em: {}", font.units_per_em);
  println!("Ascent: {}, Descent: {}", font.ascent, font.descent);
  println!("Grid height: {} pixels", font.grid_height());
  println!("Preprocessor lines: {}", font.preprocessor_lines.len());

  // Print a sample glyph as ASCII art
  if let Some(glyph) = font.glyphs.get("A") {
    println!("\nGlyph 'A' ({}x{}):", glyph.pixels.width, glyph.pixels.height);
    for row in 0..glyph.pixels.height {
      for col in 0..glyph.pixels.width {
        print!("{}", if glyph.pixels.get(col, row) { "██" } else { "░░" });
      }
      println!();
    }
  }
}
