use crate::font::{Font, Glyph, LigatureRule, PixelGrid};
use crate::ttx::write;
use crate::ui::status::StatusMessage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
  GlyphEdit,
  LigatureEdit,
  Search,
  Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchTarget {
  Glyph,
  Ligature,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatingOp {
  Paste,
  Move,
}

pub struct FloatingState {
  pub op: FloatingOp,
  pub pixels: PixelGrid,
  pub offset_row: i32,
  pub offset_col: i32,
  pub original_pixels: PixelGrid,
  pub glyph_name: String,
}

pub struct App {
  pub font: Font,
  pub file_path: String,
  pub mode: Mode,
  pub prev_mode: Mode,
  pub search_target: SearchTarget,
  pub should_quit: bool,

  // Glyph navigation
  pub glyph_index: usize,         // Index into glyph_order
  pub cursor_row: usize,
  pub cursor_col: usize,

  // Ligature navigation
  pub ligature_index: usize,

  // Search state
  pub search_input: String,
  pub search_results: Vec<String>, // display strings
  pub search_result_names: Vec<String>, // glyph names or ligature indices
  pub search_selected: usize,

  // Clipboard & floating overlay
  pub clipboard: Option<PixelGrid>,
  pub floating: Option<FloatingState>,

  // Status
  pub status_message: Option<StatusMessage>,
}

impl App {
  pub fn new(font: Font, file_path: String) -> Self {
    // Start at first glyph with a Unicode mapping (skip .notdef, control chars)
    let glyph_index = font.glyph_order.iter()
      .position(|name| {
        font.cmap_reverse.get(name)
          .map(|codes| codes.iter().any(|&c| c >= 0x20))
          .unwrap_or(false)
      })
      .unwrap_or(0);

    Self {
      font,
      file_path,
      mode: Mode::GlyphEdit,
      prev_mode: Mode::GlyphEdit,
      search_target: SearchTarget::Glyph,
      should_quit: false,
      glyph_index,
      cursor_row: 0,
      cursor_col: 0,
      ligature_index: 0,
      search_input: String::new(),
      search_results: Vec::new(),
      search_result_names: Vec::new(),
      search_selected: 0,
      clipboard: None,
      floating: None,
      status_message: None,
    }
  }

  pub fn current_glyph(&self) -> Option<&Glyph> {
    let name = self.font.glyph_order.get(self.glyph_index)?;
    self.font.glyphs.get(name)
  }

  #[allow(dead_code)]
  pub fn current_glyph_mut(&mut self) -> Option<&mut Glyph> {
    let name = self.font.glyph_order.get(self.glyph_index)?.clone();
    self.font.glyphs.get_mut(&name)
  }

  pub fn current_glyph_name(&self) -> Option<&str> {
    self.font.glyph_order.get(self.glyph_index).map(|s| s.as_str())
  }

  pub fn current_ligature(&self) -> Option<&LigatureRule> {
    self.font.ligatures.get(self.ligature_index)
  }

  // -- Navigation --

  pub fn next_glyph(&mut self) {
    if self.glyph_index + 1 < self.font.glyph_order.len() {
      self.glyph_index += 1;
      self.clamp_cursor();
    }
  }

  pub fn prev_glyph(&mut self) {
    if self.glyph_index > 0 {
      self.glyph_index -= 1;
      self.clamp_cursor();
    }
  }

  pub fn next_ligature(&mut self) {
    if self.ligature_index + 1 < self.font.ligatures.len() {
      self.ligature_index += 1;
      self.clamp_cursor_ligature();
    }
  }

  pub fn prev_ligature(&mut self) {
    if self.ligature_index > 0 {
      self.ligature_index -= 1;
      self.clamp_cursor_ligature();
    }
  }

  fn clamp_cursor(&mut self) {
    if let Some(g) = self.current_glyph() {
      let max_col = g.pixels.width.saturating_sub(1);
      let max_row = g.pixels.height.saturating_sub(1);
      self.cursor_col = self.cursor_col.min(max_col);
      self.cursor_row = self.cursor_row.min(max_row);
    }
  }

  fn clamp_cursor_ligature(&mut self) {
    if let Some(lig) = self.current_ligature() {
      if let Some(g) = self.font.glyphs.get(&lig.result_glyph) {
        let max_col = g.pixels.width.saturating_sub(1);
        let max_row = g.pixels.height.saturating_sub(1);
        self.cursor_col = self.cursor_col.min(max_col);
        self.cursor_row = self.cursor_row.min(max_row);
      }
    }
  }

  pub fn move_cursor(&mut self, dr: i32, dc: i32) {
    let (max_col, max_row) = match self.mode {
      Mode::GlyphEdit => {
        if let Some(g) = self.current_glyph() {
          (g.pixels.width.saturating_sub(1), g.pixels.height.saturating_sub(1))
        } else {
          return;
        }
      }
      Mode::LigatureEdit => {
        if let Some(lig) = self.current_ligature() {
          if let Some(g) = self.font.glyphs.get(&lig.result_glyph) {
            (g.pixels.width.saturating_sub(1), g.pixels.height.saturating_sub(1))
          } else {
            return;
          }
        } else {
          return;
        }
      }
      _ => return,
    };

    let new_row = (self.cursor_row as i32 + dr).clamp(0, max_row as i32) as usize;
    let new_col = (self.cursor_col as i32 + dc).clamp(0, max_col as i32) as usize;
    self.cursor_row = new_row;
    self.cursor_col = new_col;
  }

  // -- Editing --

  pub fn toggle_pixel(&mut self) {
    let name = match self.mode {
      Mode::GlyphEdit => {
        match self.current_glyph_name() {
          Some(n) => n.to_string(),
          None => return,
        }
      }
      Mode::LigatureEdit => {
        match self.current_ligature() {
          Some(l) => l.result_glyph.clone(),
          None => return,
        }
      }
      _ => return,
    };

    if let Some(glyph) = self.font.glyphs.get_mut(&name) {
      glyph.pixels.toggle(self.cursor_col, self.cursor_row);
      self.font.dirty.insert(name);
    }
  }

  // -- Clipboard & floating overlay --

  /// Get the glyph name being edited in the current mode.
  fn editing_glyph_name(&self) -> Option<String> {
    match self.mode {
      Mode::GlyphEdit => self.current_glyph_name().map(|s| s.to_string()),
      Mode::LigatureEdit => self.current_ligature().map(|l| l.result_glyph.clone()),
      _ => None,
    }
  }

  pub fn copy(&mut self) {
    let name = match self.editing_glyph_name() {
      Some(n) => n,
      None => return,
    };
    if let Some(glyph) = self.font.glyphs.get(&name) {
      self.clipboard = Some(glyph.pixels.clone());
      self.status_message = Some(StatusMessage::new("Copied to clipboard".to_string()));
    }
  }

  pub fn start_paste(&mut self) {
    let clip = match &self.clipboard {
      Some(c) => c.clone(),
      None => {
        self.status_message = Some(StatusMessage::new("Clipboard is empty".to_string()));
        return;
      }
    };
    let name = match self.editing_glyph_name() {
      Some(n) => n,
      None => return,
    };
    let original = match self.font.glyphs.get(&name) {
      Some(g) => g.pixels.clone(),
      None => return,
    };
    self.floating = Some(FloatingState {
      op: FloatingOp::Paste,
      pixels: clip,
      offset_row: 0,
      offset_col: 0,
      original_pixels: original,
      glyph_name: name,
    });
    self.status_message = Some(StatusMessage::new(
      "Paste: arrows to position, Enter to commit, Esc to cancel".to_string()
    ));
  }

  pub fn start_move(&mut self) {
    let name = match self.editing_glyph_name() {
      Some(n) => n,
      None => return,
    };
    let glyph = match self.font.glyphs.get_mut(&name) {
      Some(g) => g,
      None => return,
    };
    let original = glyph.pixels.clone();
    let lifted = glyph.pixels.clone();
    // Clear the canvas — pixels are now in the floating layer
    glyph.pixels = PixelGrid::new(glyph.pixels.width, glyph.pixels.height);
    self.floating = Some(FloatingState {
      op: FloatingOp::Move,
      pixels: lifted,
      offset_row: 0,
      offset_col: 0,
      original_pixels: original,
      glyph_name: name,
    });
    self.status_message = Some(StatusMessage::new(
      "Move: arrows to position, Enter to commit, Esc to cancel".to_string()
    ));
  }

  pub fn reset_canvas(&mut self) {
    let name = match self.editing_glyph_name() {
      Some(n) => n,
      None => return,
    };
    if let Some(glyph) = self.font.glyphs.get_mut(&name) {
      glyph.pixels = PixelGrid::new(glyph.pixels.width, glyph.pixels.height);
      self.font.dirty.insert(name);
      self.status_message = Some(StatusMessage::new("Canvas cleared".to_string()));
    }
  }

  pub fn move_floating(&mut self, dr: i32, dc: i32) {
    let fl = match &mut self.floating {
      Some(f) => f,
      None => return,
    };
    let new_row = fl.offset_row + dr;
    let new_col = fl.offset_col + dc;
    if fl.op == FloatingOp::Move {
      // Clamp so the bounding box of filled pixels stays within grid bounds
      if let Some((min_r, max_r, min_c, max_c)) = filled_bbox(&fl.pixels) {
        let canvas_h = fl.original_pixels.height as i32;
        let canvas_w = fl.original_pixels.width as i32;
        let clamped_r = new_row.max(-(min_r as i32)).min(canvas_h - 1 - max_r as i32);
        let clamped_c = new_col.max(-(min_c as i32)).min(canvas_w - 1 - max_c as i32);
        fl.offset_row = clamped_r;
        fl.offset_col = clamped_c;
      }
    } else {
      // Paste: no clamping, can go partially outside
      fl.offset_row = new_row;
      fl.offset_col = new_col;
    }
  }

  pub fn commit_floating(&mut self) {
    let fl = match self.floating.take() {
      Some(f) => f,
      None => return,
    };
    if let Some(glyph) = self.font.glyphs.get_mut(&fl.glyph_name) {
      // OR floating pixels onto canvas (only within bounds)
      for r in 0..fl.pixels.height {
        for c in 0..fl.pixels.width {
          if fl.pixels.get(c, r) {
            let cr = r as i32 + fl.offset_row;
            let cc = c as i32 + fl.offset_col;
            if cr >= 0 && cr < glyph.pixels.height as i32
              && cc >= 0 && cc < glyph.pixels.width as i32
            {
              glyph.pixels.set(cc as usize, cr as usize, true);
            }
          }
        }
      }
      self.font.dirty.insert(fl.glyph_name);
    }
    self.status_message = Some(StatusMessage::new("Committed".to_string()));
  }

  pub fn rotate_floating(&mut self) {
    let fl = match &mut self.floating {
      Some(f) => f,
      None => return,
    };
    fl.pixels = fl.pixels.rotate_cw();
  }

  pub fn cancel_floating(&mut self) {
    let fl = match self.floating.take() {
      Some(f) => f,
      None => return,
    };
    // Restore original pixels
    if let Some(glyph) = self.font.glyphs.get_mut(&fl.glyph_name) {
      glyph.pixels = fl.original_pixels;
    }
    self.status_message = Some(StatusMessage::new("Cancelled".to_string()));
  }

  pub fn save(&mut self) {
    match write::save_ttx(&mut self.font, &self.file_path) {
      Ok(()) => {
        let count = self.font.dirty.len();
        self.font.dirty.clear();
        self.status_message = Some(StatusMessage::new(
          format!("Saved {} modified glyph(s) to {}", count, self.file_path)
        ));
      }
      Err(e) => {
        self.status_message = Some(StatusMessage::new(format!("Save failed: {e}")));
      }
    }
  }

  pub fn export_svg(&mut self) {
    let name = match self.editing_glyph_name() {
      Some(n) => n,
      None => return,
    };
    let glyph = match self.font.glyphs.get(&name) {
      Some(g) => g,
      None => return,
    };
    let svg = generate_glyph_svg(&glyph.pixels, self.font.ascent);
    let filename = format!("{}.svg", name);
    match std::fs::write(&filename, &svg) {
      Ok(()) => {
        self.status_message = Some(StatusMessage::new(
          format!("Exported {}", filename)
        ));
      }
      Err(e) => {
        self.status_message = Some(StatusMessage::new(
          format!("Export failed: {e}")
        ));
      }
    }
  }

  // -- Search --

  pub fn open_search(&mut self, target: SearchTarget) {
    self.prev_mode = self.mode;
    self.mode = Mode::Search;
    self.search_target = target;
    self.search_input.clear();
    self.search_results.clear();
    self.search_result_names.clear();
    self.search_selected = 0;
  }

  pub fn close_search(&mut self) {
    self.mode = self.prev_mode;
  }

  pub fn update_search(&mut self) {
    self.search_results.clear();
    self.search_result_names.clear();
    self.search_selected = 0;

    let query = self.search_input.trim().to_string();
    if query.is_empty() {
      return;
    }

    match self.search_target {
      SearchTarget::Glyph => self.search_glyphs(&query),
      SearchTarget::Ligature => self.search_ligatures(&query),
    }
  }

  fn search_glyphs(&mut self, query: &str) {
    let query_lower = query.to_lowercase();

    // Try exact codepoint (U+XXXX or 0xXXXX)
    if let Some(cp) = parse_codepoint(query) {
      if let Some(name) = self.font.cmap.get(&cp) {
        let ch = char::from_u32(cp).map(|c| format!(" \"{}\"", c)).unwrap_or_default();
        self.search_results.push(format!("U+{:04X}{} → {}", cp, ch, name));
        self.search_result_names.push(name.clone());
      } else {
        let ch = char::from_u32(cp).map(|c| format!(" \"{}\"", c)).unwrap_or_default();
        let uname = char::from_u32(cp)
          .and_then(|c| unicode_names2::name(c))
          .map(|n| format!(" {}", n))
          .unwrap_or_default();
        self.search_results.push(
          format!("+ Create glyph U+{:04X}{}{}", cp, ch, uname)
        );
        self.search_result_names.push(format!("__create__:{}", cp));
      }
      return;
    }

    // Try single character literal
    if query.chars().count() == 1 {
      let ch = query.chars().next().unwrap();
      let cp = ch as u32;
      if let Some(name) = self.font.cmap.get(&cp) {
        let uname = unicode_names2::name(ch)
          .map(|n| format!(" {}", n))
          .unwrap_or_default();
        self.search_results.push(format!("U+{:04X} \"{}\"{} → {}", cp, ch, uname, name));
        self.search_result_names.push(name.clone());
        return;
      } else {
        let uname = unicode_names2::name(ch)
          .map(|n| format!(" {}", n))
          .unwrap_or_default();
        self.search_results.push(
          format!("+ Create glyph U+{:04X} \"{}\"{}", cp, ch, uname)
        );
        self.search_result_names.push(format!("__create__:{}", cp));
        return;
      }
    }

    // Search glyph names
    for name in &self.font.glyph_order {
      if name.to_lowercase().contains(&query_lower) {
        let codes = self.font.cmap_reverse.get(name);
        let code_str = codes
          .and_then(|cs| cs.first())
          .map(|&c| format!("U+{:04X} ", c))
          .unwrap_or_default();
        self.search_results.push(format!("{}{}", code_str, name));
        self.search_result_names.push(name.clone());
        if self.search_results.len() >= 50 {
          break;
        }
      }
    }
    if !self.search_results.is_empty() {
      return;
    }

    // Search Unicode names
    for (&cp, name) in &self.font.cmap {
      if let Some(ch) = char::from_u32(cp) {
        if let Some(uname) = unicode_names2::name(ch) {
          let uname_str = uname.to_string();
          if uname_str.to_lowercase().contains(&query_lower) {
            self.search_results.push(format!("U+{:04X} \"{}\" {} → {}", cp, ch, uname_str, name));
            self.search_result_names.push(name.clone());
            if self.search_results.len() >= 50 {
              break;
            }
          }
        }
      }
    }
  }

  fn search_ligatures(&mut self, query: &str) {
    let query_lower = query.to_lowercase();

    // Try matching trigger text
    for (i, lig) in self.font.ligatures.iter().enumerate() {
      let trigger = lig.trigger_text(&self.font.cmap_reverse);
      if trigger.contains(query) || trigger.to_lowercase().contains(&query_lower) {
        self.search_results.push(format!("\"{}\" → {}", trigger, lig.result_glyph));
        self.search_result_names.push(i.to_string());
        if self.search_results.len() >= 50 {
          return;
        }
      }
    }

    // Search result glyph names
    for (i, lig) in self.font.ligatures.iter().enumerate() {
      if lig.result_glyph.to_lowercase().contains(&query_lower) {
        let trigger = lig.trigger_text(&self.font.cmap_reverse);
        let entry = format!("\"{}\" → {}", trigger, lig.result_glyph);
        if !self.search_results.contains(&entry) {
          self.search_results.push(entry);
          self.search_result_names.push(i.to_string());
          if self.search_results.len() >= 50 {
            return;
          }
        }
      }
    }
  }

  pub fn select_search_result(&mut self) {
    if self.search_selected >= self.search_result_names.len() {
      self.close_search();
      return;
    }

    let name = self.search_result_names[self.search_selected].clone();

    // Handle "create new glyph" entries
    if let Some(cp_str) = name.strip_prefix("__create__:") {
      if let Ok(cp) = cp_str.parse::<u32>() {
        self.create_glyph(cp);
        return;
      }
    }

    match self.search_target {
      SearchTarget::Glyph => {
        if let Some(idx) = self.font.glyph_order.iter().position(|n| n == &name) {
          self.glyph_index = idx;
          self.cursor_row = 0;
          self.cursor_col = 0;
          self.mode = Mode::GlyphEdit;
        }
      }
      SearchTarget::Ligature => {
        if let Ok(idx) = name.parse::<usize>() {
          self.ligature_index = idx;
          self.cursor_row = 0;
          self.cursor_col = 0;
          self.mode = Mode::LigatureEdit;
        }
      }
    }
  }

  /// Create a new empty glyph for the given Unicode codepoint.
  fn create_glyph(&mut self, cp: u32) {
    use crate::font::{HmtxEntry, PixelGrid};

    // Generate glyph name: uniXXXX for BMP, uXXXXXX for supplementary
    let glyph_name = if cp <= 0xFFFF {
      format!("uni{:04X}", cp)
    } else {
      format!("u{:06X}", cp)
    };

    let grid_w = self.font.grid_width(80);
    let grid_h = self.font.grid_height();

    let glyph = Glyph {
      name: glyph_name.clone(),
      pixels: PixelGrid::new(grid_w, grid_h),
      contours: Vec::new(),
      width: 80,
      lsb: 0,
      bbox: None,
    };

    self.font.glyph_order.push(glyph_name.clone());
    self.font.glyphs.insert(glyph_name.clone(), glyph);
    self.font.cmap.insert(cp, glyph_name.clone());
    self.font.cmap_reverse.entry(glyph_name.clone()).or_default().push(cp);
    self.font.hmtx.insert(glyph_name.clone(), HmtxEntry { width: 80, lsb: 0 });
    self.font.dirty.insert(glyph_name);

    self.glyph_index = self.font.glyph_order.len() - 1;
    self.cursor_row = 0;
    self.cursor_col = 0;
    self.mode = Mode::GlyphEdit;

    let ch_display = char::from_u32(cp)
      .map(|c| format!(" \"{}\"", c))
      .unwrap_or_default();
    self.status_message = Some(StatusMessage::new(
      format!("Created new glyph for U+{:04X}{}", cp, ch_display)
    ));
  }
}

/// Find the bounding box of filled pixels. Returns (min_row, max_row, min_col, max_col).
fn filled_bbox(grid: &PixelGrid) -> Option<(usize, usize, usize, usize)> {
  let mut min_r = usize::MAX;
  let mut max_r = 0;
  let mut min_c = usize::MAX;
  let mut max_c = 0;
  let mut found = false;
  for r in 0..grid.height {
    for c in 0..grid.width {
      if grid.get(c, r) {
        found = true;
        min_r = min_r.min(r);
        max_r = max_r.max(r);
        min_c = min_c.min(c);
        max_c = max_c.max(c);
      }
    }
  }
  if found { Some((min_r, max_r, min_c, max_c)) } else { None }
}

fn generate_glyph_svg(grid: &PixelGrid, ascent: i32) -> String {
  use crate::grid;
  let (contours, _) = grid::vectorize(grid, ascent);

  if contours.is_empty() {
    return format!(
      "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\"/>\n",
      grid.width, grid.height,
    );
  }

  let mut path_data = String::new();
  for contour in &contours {
    if contour.points.is_empty() { continue; }
    let first = &contour.points[0];
    // Font coords → pixel coords: col = x/10, row = (ascent - y) / 10
    path_data.push_str(&format!(
      "M{} {}", first.x / 10, (ascent - first.y) / 10
    ));
    for pt in &contour.points[1..] {
      path_data.push_str(&format!(
        " L{} {}", pt.x / 10, (ascent - pt.y) / 10
      ));
    }
    path_data.push_str(" Z ");
  }

  format!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">\n\
     \x20 <path d=\"{}\" fill=\"black\" fill-rule=\"nonzero\"/>\n\
     </svg>\n",
    grid.width, grid.height,
    path_data.trim(),
  )
}

fn parse_codepoint(s: &str) -> Option<u32> {
  let s = s.trim();
  if let Some(hex) = s.strip_prefix("U+").or_else(|| s.strip_prefix("u+"))
    .or_else(|| s.strip_prefix("0x")).or_else(|| s.strip_prefix("0X"))
  {
    u32::from_str_radix(hex, 16).ok()
  } else {
    None
  }
}
