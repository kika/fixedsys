use std::collections::{BTreeMap, HashMap, HashSet};

/// A single point in a glyph contour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
  pub x: i32,
  pub y: i32,
}

/// A closed contour (list of on-curve points, all rectilinear).
#[derive(Debug, Clone)]
pub struct Contour {
  pub points: Vec<Point>,
}

/// Bounding box for a glyph.
#[derive(Debug, Clone, Copy)]
pub struct BBox {
  pub x_min: i32,
  pub y_min: i32,
  pub x_max: i32,
  pub y_max: i32,
}

/// Row-major pixel grid for bitmap editing.
#[derive(Debug, Clone)]
pub struct PixelGrid {
  pub width: usize,
  pub height: usize,
  pub data: Vec<bool>,
}

impl PixelGrid {
  pub fn new(width: usize, height: usize) -> Self {
    Self {
      width,
      height,
      data: vec![false; width * height],
    }
  }

  pub fn get(&self, col: usize, row: usize) -> bool {
    if col < self.width && row < self.height {
      self.data[row * self.width + col]
    } else {
      false
    }
  }

  pub fn set(&mut self, col: usize, row: usize, val: bool) {
    if col < self.width && row < self.height {
      self.data[row * self.width + col] = val;
    }
  }

  pub fn toggle(&mut self, col: usize, row: usize) {
    if col < self.width && row < self.height {
      let idx = row * self.width + col;
      self.data[idx] = !self.data[idx];
    }
  }

  /// Rotate 90° clockwise. New dimensions: width=old height, height=old width.
  /// Original (c, r) maps to new (height-1-r, c).
  pub fn rotate_cw(&self) -> Self {
    let new_w = self.height;
    let new_h = self.width;
    let mut out = PixelGrid::new(new_w, new_h);
    for r in 0..self.height {
      for c in 0..self.width {
        if self.get(c, r) {
          out.set(self.height - 1 - r, c, true);
        }
      }
    }
    out
  }
}

/// Horizontal metrics for a glyph.
#[derive(Debug, Clone, Copy)]
pub struct HmtxEntry {
  pub width: i32,
  pub lsb: i32,
}

/// A single glyph with both contour and pixel representations.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used by vectorizer on save
pub struct Glyph {
  pub name: String,
  pub pixels: PixelGrid,
  pub contours: Vec<Contour>,
  pub width: i32,
  pub lsb: i32,
  pub bbox: Option<BBox>,
}

/// A GSUB ligature rule: first_glyph + components → result_glyph.
#[derive(Debug, Clone)]
pub struct LigatureRule {
  pub first_glyph: String,
  pub components: Vec<String>,
  pub result_glyph: String,
}

impl LigatureRule {
  /// Return the trigger string for display (e.g., "->", "<=").
  pub fn trigger_text(&self, cmap_reverse: &HashMap<String, Vec<u32>>) -> String {
    let mut parts = vec![self.first_glyph.clone()];
    parts.extend(self.components.iter().cloned());
    parts
      .iter()
      .map(|name| {
        cmap_reverse
          .get(name)
          .and_then(|codes| codes.first())
          .and_then(|&c| char::from_u32(c))
          .map(|c| c.to_string())
          .unwrap_or_else(|| name.clone())
      })
      .collect()
  }
}

/// A preprocessor directive line preserved for round-trip save.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields kept for potential future use (new glyph creation in GSUB)
pub struct PreprocessorLine {
  pub line_number: usize,
  pub content: String,
}

/// Line range in the source file (1-based, inclusive).
#[derive(Debug, Clone, Copy)]
pub struct LineRange {
  pub start: usize,
  pub end: usize,
}

/// The complete parsed font.
pub struct Font {
  pub glyph_order: Vec<String>,
  pub glyphs: HashMap<String, Glyph>,
  pub cmap: BTreeMap<u32, String>,
  pub cmap_reverse: HashMap<String, Vec<u32>>,
  pub hmtx: HashMap<String, HmtxEntry>,
  pub ligatures: Vec<LigatureRule>,
  pub units_per_em: i32,
  pub ascent: i32,
  pub descent: i32,
  pub dirty: HashSet<String>,
  pub source_lines: Vec<String>,
  pub preprocessor_lines: Vec<PreprocessorLine>,
  // Line ranges for surgical save
  pub glyph_order_range: Option<LineRange>,
  pub hmtx_range: Option<LineRange>,
  pub glyf_ranges: HashMap<String, LineRange>,
  pub glyf_section_end: Option<usize>, // line number of </glyf>
  pub gsub_lookup5_range: Option<LineRange>,
  pub cmap_format4_range: Option<LineRange>,
}

impl Font {
  pub fn new() -> Self {
    Self {
      glyph_order: Vec::new(),
      glyphs: HashMap::new(),
      cmap: BTreeMap::new(),
      cmap_reverse: HashMap::new(),
      hmtx: HashMap::new(),
      ligatures: Vec::new(),
      units_per_em: 160,
      ascent: 130,
      descent: -30,
      dirty: HashSet::new(),
      source_lines: Vec::new(),
      preprocessor_lines: Vec::new(),
      glyph_order_range: None,
      hmtx_range: None,
      glyf_ranges: HashMap::new(),
      glyf_section_end: None,
      gsub_lookup5_range: None,
      cmap_format4_range: None,
    }
  }

  /// Get the pixel grid height (always 16 for this font).
  pub fn grid_height(&self) -> usize {
    ((self.ascent - self.descent) / 10) as usize
  }

  /// Get the pixel grid width for a given advance width.
  pub fn grid_width(&self, advance_width: i32) -> usize {
    (advance_width / 10) as usize
  }

  /// Look up a glyph name by Unicode codepoint.
  #[allow(dead_code)]
  pub fn glyph_for_codepoint(&self, cp: u32) -> Option<&str> {
    self.cmap.get(&cp).map(|s| s.as_str())
  }

  #[allow(dead_code)]
  pub fn unicode_glyph_names(&self) -> Vec<&str> {
    self.glyph_order
      .iter()
      .filter(|name| self.cmap_reverse.contains_key(name.as_str()))
      .map(|s| s.as_str())
      .collect()
  }
}
