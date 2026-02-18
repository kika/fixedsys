use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::font::*;
use crate::grid;

/// Parse a TTX file into a Font structure.
pub fn parse_ttx(path: &str) -> Result<Font> {
  let raw = std::fs::read_to_string(path).context("reading TTX file")?;
  let mut font = Font::new();

  // Store original lines for surgical save
  font.source_lines = raw.lines().map(|l| l.to_string()).collect();
  let total_lines = font.source_lines.len();

  // Strip preprocessor lines, record their positions (1-based)
  let mut clean_lines: Vec<String> = Vec::with_capacity(total_lines);
  for (i, line) in font.source_lines.iter().enumerate() {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') && !trimmed.starts_with("<?") {
      font.preprocessor_lines.push(PreprocessorLine {
        line_number: i + 1, // 1-based
        content: line.clone(),
      });
      // Replace with blank line to preserve line numbering
      clean_lines.push(String::new());
    } else {
      clean_lines.push(line.clone());
    }
  }

  let clean_xml = clean_lines.join("\n");
  let mut reader = Reader::from_str(&clean_xml);

  // Track current byte offset → line number mapping
  // We'll use byte offsets to track positions
  let line_offsets = build_line_offsets(&clean_xml);

  let mut current_glyph_name = String::new();
  let mut current_contour: Vec<Point> = Vec::new();
  let mut current_contours: Vec<Contour> = Vec::new();
  let mut current_glyph_bbox: Option<BBox> = None;
  let mut current_glyph_start_line: usize = 0;

  // GSUB state
  let mut in_gsub = false;
  let mut in_lookup5 = false;
  let mut in_ligature_subst = false;
  let mut current_ligature_set_glyph = String::new();
  let mut lookup_index: Option<i32>;
  let mut gsub_lookup5_start_line: usize = 0;

  // cmap state
  let mut in_cmap = false;
  let mut in_cmap_format4 = false;
  let mut cmap_format4_start: usize = 0;

  // Section tracking
  let mut glyph_order_start: usize = 0;
  let mut hmtx_start: usize = 0;
  let mut in_glyph_order = false;
  let mut in_hmtx = false;
  let mut in_head = false;
  let mut in_hhea = false;
  let mut in_glyf = false;

  loop {
    let pos = reader.buffer_position();
    let event = reader.read_event();
    let is_empty_event = matches!(&event, Ok(Event::Empty(_)));

    match event {
      Ok(Event::Eof) => break,
      Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
        let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
        let line = offset_to_line(&line_offsets, pos as usize);

        match tag.as_str() {
          "GlyphOrder" => {
            in_glyph_order = true;
            glyph_order_start = line;
          }
          "GlyphID" if in_glyph_order => {
            if let Some(name) = get_attr(e, "name") {
              font.glyph_order.push(name);
            }
          }
          "head" => { in_head = true; }
          "unitsPerEm" if in_head => {
            if let Some(v) = get_attr(e, "value") {
              font.units_per_em = v.parse().unwrap_or(160);
            }
          }
          "hhea" => { in_hhea = true; }
          "ascent" if in_hhea => {
            if let Some(v) = get_attr(e, "value") {
              font.ascent = v.parse().unwrap_or(130);
            }
          }
          "descent" if in_hhea => {
            if let Some(v) = get_attr(e, "value") {
              font.descent = v.parse().unwrap_or(-30);
            }
          }
          "hmtx" => {
            in_hmtx = true;
            hmtx_start = line;
          }
          "mtx" if in_hmtx => {
            if let (Some(name), Some(w), Some(lsb)) =
              (get_attr(e, "name"), get_attr(e, "width"), get_attr(e, "lsb"))
            {
              font.hmtx.insert(name, HmtxEntry {
                width: w.parse().unwrap_or(80),
                lsb: lsb.parse().unwrap_or(0),
              });
            }
          }
          "cmap" => { in_cmap = true; }
          "cmap_format_4" if in_cmap => {
            let pid = get_attr(e, "platformID").unwrap_or_default();
            let eid = get_attr(e, "platEncID").unwrap_or_default();
            if pid == "3" && eid == "1" {
              in_cmap_format4 = true;
              cmap_format4_start = line;
            }
          }
          "map" if in_cmap_format4 => {
            if let (Some(code_str), Some(name)) = (get_attr(e, "code"), get_attr(e, "name")) {
              if let Some(code) = parse_hex_or_dec(&code_str) {
                font.cmap.insert(code, name.clone());
                font.cmap_reverse.entry(name).or_default().push(code);
              }
            }
          }
          "glyf" => { in_glyf = true; }
          "TTGlyph" if in_glyf => {
            current_glyph_name = get_attr(e, "name").unwrap_or_default();
            current_contours.clear();
            current_glyph_start_line = line;
            current_glyph_bbox = None;
            if let (Some(xmin), Some(ymin), Some(xmax), Some(ymax)) = (
              get_attr(e, "xMin").and_then(|v| v.parse::<i32>().ok()),
              get_attr(e, "yMin").and_then(|v| v.parse::<i32>().ok()),
              get_attr(e, "xMax").and_then(|v| v.parse::<i32>().ok()),
              get_attr(e, "yMax").and_then(|v| v.parse::<i32>().ok()),
            ) {
              current_glyph_bbox = Some(BBox {
                x_min: xmin, y_min: ymin, x_max: xmax, y_max: ymax,
              });
            }
          }
          "contour" if in_glyf => {
            current_contour.clear();
          }
          "pt" if in_glyf => {
            if let (Some(x), Some(y)) = (
              get_attr(e, "x").and_then(|v| v.parse::<i32>().ok()),
              get_attr(e, "y").and_then(|v| v.parse::<i32>().ok()),
            ) {
              current_contour.push(Point { x, y });
            }
          }
          "GSUB" => { in_gsub = true; }
          "Lookup" if in_gsub => {
            lookup_index = get_attr(e, "index").and_then(|v| v.parse().ok());
            if lookup_index == Some(5) {
              in_lookup5 = true;
              gsub_lookup5_start_line = line;
            }
          }
          "LigatureSubst" if in_lookup5 => {
            in_ligature_subst = true;
          }
          "LigatureSet" if in_ligature_subst => {
            current_ligature_set_glyph = get_attr(e, "glyph").unwrap_or_default();
          }
          "Ligature" if in_ligature_subst => {
            if let (Some(components_str), Some(result_glyph)) =
              (get_attr(e, "components"), get_attr(e, "glyph"))
            {
              let components: Vec<String> =
                components_str.split(',').map(|s| s.trim().to_string()).collect();
              font.ligatures.push(LigatureRule {
                first_glyph: current_ligature_set_glyph.clone(),
                components,
                result_glyph,
              });
            }
          }
          _ => {}
        }

        // Handle self-closing TTGlyph (empty glyphs like <TTGlyph name="space"/>)
        if is_empty_event && tag == "TTGlyph" && in_glyf && !current_glyph_name.is_empty() {
          let name = std::mem::take(&mut current_glyph_name);
          let hmtx_entry = font.hmtx.get(&name).copied()
            .unwrap_or(HmtxEntry { width: 80, lsb: 0 });
          let grid_w = font.grid_width(hmtx_entry.width);
          let grid_h = font.grid_height();
          font.glyf_ranges.insert(name.clone(), LineRange {
            start: current_glyph_start_line,
            end: line,
          });
          font.glyphs.insert(name.clone(), Glyph {
            name,
            pixels: PixelGrid::new(grid_w, grid_h),
            contours: Vec::new(),
            width: hmtx_entry.width,
            lsb: hmtx_entry.lsb,
            bbox: None,
          });
        }
      }
      Ok(Event::End(ref e)) => {
        let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
        let line = offset_to_line(&line_offsets, pos as usize);

        match tag.as_str() {
          "GlyphOrder" => {
            in_glyph_order = false;
            font.glyph_order_range = Some(LineRange {
              start: glyph_order_start,
              end: line,
            });
          }
          "head" => { in_head = false; }
          "hhea" => { in_hhea = false; }
          "hmtx" => {
            in_hmtx = false;
            font.hmtx_range = Some(LineRange {
              start: hmtx_start,
              end: line,
            });
          }
          "cmap_format_4" if in_cmap_format4 => {
            in_cmap_format4 = false;
            font.cmap_format4_range = Some(LineRange {
              start: cmap_format4_start,
              end: line,
            });
          }
          "cmap" => { in_cmap = false; }
          "contour" if in_glyf => {
            if !current_contour.is_empty() {
              current_contours.push(Contour {
                points: std::mem::take(&mut current_contour),
              });
            }
          }
          "TTGlyph" if in_glyf => {
            let name = std::mem::take(&mut current_glyph_name);
            let contours = std::mem::take(&mut current_contours);
            let hmtx_entry = font.hmtx.get(&name).copied().unwrap_or(HmtxEntry { width: 80, lsb: 0 });
            let grid_w = font.grid_width(hmtx_entry.width);
            let grid_h = font.grid_height();
            let pixels = grid::rasterize(&contours, grid_w, grid_h, font.ascent);

            font.glyf_ranges.insert(name.clone(), LineRange {
              start: current_glyph_start_line,
              end: line,
            });

            font.glyphs.insert(name.clone(), Glyph {
              name,
              pixels,
              contours,
              width: hmtx_entry.width,
              lsb: hmtx_entry.lsb,
              bbox: current_glyph_bbox,
            });
          }
          "glyf" => {
            in_glyf = false;
            font.glyf_section_end = Some(line);
          }
          "Lookup" if in_lookup5 => {
            in_lookup5 = false;
            in_ligature_subst = false;
            font.gsub_lookup5_range = Some(LineRange {
              start: gsub_lookup5_start_line,
              end: line,
            });
          }
          "LigatureSubst" if in_ligature_subst => {
            in_ligature_subst = false;
          }
          "LigatureSet" if in_ligature_subst => {
            current_ligature_set_glyph.clear();
          }
          "GSUB" => { in_gsub = false; }
          _ => {}
        }
      }
      Err(e) => {
        // Skip XML errors (may be caused by preprocessor line removal)
        eprintln!("XML parse warning at position {}: {:?}", pos, e);
      }
      _ => {}
    }
  }

  Ok(font)
}

/// Build a list of byte offsets for the start of each line (0-indexed).
fn build_line_offsets(text: &str) -> Vec<usize> {
  let mut offsets = vec![0usize];
  for (i, b) in text.bytes().enumerate() {
    if b == b'\n' {
      offsets.push(i + 1);
    }
  }
  offsets
}

/// Convert byte offset to 1-based line number.
fn offset_to_line(line_offsets: &[usize], offset: usize) -> usize {
  match line_offsets.binary_search(&offset) {
    Ok(i) => i + 1,
    Err(i) => i, // offset is within line i (0-indexed), so 1-based = i
  }
}

/// Get an attribute value from an XML element.
fn get_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
  for attr in e.attributes().flatten() {
    if attr.key.as_ref() == name.as_bytes() {
      return Some(String::from_utf8_lossy(&attr.value).to_string());
    }
  }
  None
}

/// Parse "0x41" or "65" into u32.
fn parse_hex_or_dec(s: &str) -> Option<u32> {
  if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
    u32::from_str_radix(hex, 16).ok()
  } else {
    s.parse().ok()
  }
}
