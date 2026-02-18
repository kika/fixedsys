use anyhow::{Context, Result};

use crate::font::*;
use crate::grid;

/// Save the font back to a TTX file using surgical line replacement.
/// Only modified glyphs are rewritten; everything else is preserved verbatim.
/// New glyphs are inserted at the end of each relevant section.
/// Preprocessor directives are already in source_lines and left untouched.
///
/// After writing, updates source_lines and rescans all line ranges so
/// subsequent saves operate on the correct baseline.
pub fn save_ttx(font: &mut Font, path: &str) -> Result<()> {
  let mut lines = font.source_lines.clone();

  // Collect all replacements: (start_line, end_line, new_lines)
  let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
  // Collect new glyphs (dirty but no glyf_ranges entry)
  let mut new_glyphs: Vec<String> = Vec::new();

  for name in &font.dirty {
    let glyph = match font.glyphs.get(name) {
      Some(g) => g,
      None => continue,
    };

    // Regenerate contours from pixels
    let (contours, bbox) = grid::vectorize(&glyph.pixels, font.ascent);

    if let Some(range) = font.glyf_ranges.get(name) {
      let new_lines = generate_ttglyph_xml(name, &contours, bbox.as_ref());
      replacements.push((range.start, range.end, new_lines));
    } else {
      new_glyphs.push(name.clone());
    }
  }

  // Sort by start line descending — process bottom-to-top so line shifts
  // from earlier replacements don't affect later ones
  replacements.sort_by(|a, b| b.0.cmp(&a.0));

  for (start, end, new_lines) in replacements {
    replace_lines(&mut lines, start, end, &new_lines);
  }

  // Insert new glyphs at the end of each relevant section.
  // Process bottom-to-top so earlier insertions don't shift later ones.
  if !new_glyphs.is_empty() {
    // Gather all insertions: (line_before_which_to_insert, lines_to_insert)
    let mut insertions: Vec<(usize, Vec<String>)> = Vec::new();

    for name in &new_glyphs {
      let glyph = font.glyphs.get(name).unwrap();
      let (contours, bbox) = grid::vectorize(&glyph.pixels, font.ascent);

      // Insert TTGlyph before </glyf>
      if let Some(glyf_end) = font.glyf_section_end {
        let glyph_xml = generate_ttglyph_xml(name, &contours, bbox.as_ref());
        insertions.push((glyf_end, glyph_xml));
      }

      // Insert <mtx> before </hmtx>
      if let Some(ref hmtx_range) = font.hmtx_range {
        let hmtx_line = format!(
          "    <mtx name=\"{}\" width=\"{}\" lsb=\"{}\"/>",
          name, glyph.width, glyph.lsb
        );
        insertions.push((hmtx_range.end, vec![hmtx_line]));
      }

      // Insert <GlyphID> before </GlyphOrder>
      if let Some(ref go_range) = font.glyph_order_range {
        let gid = font.glyph_order.len() - new_glyphs.len(); // approximate
        let go_line = format!(
          "    <GlyphID id=\"{}\" name=\"{}\"/>",
          gid, name
        );
        insertions.push((go_range.end, vec![go_line]));
      }

      // Insert <map> into cmap_format_4
      if let Some(ref cmap_range) = font.cmap_format4_range {
        if let Some(cps) = font.cmap_reverse.get(name) {
          for &cp in cps {
            let map_line = format!(
              "      <map code=\"0x{:04x}\" name=\"{}\"/><!-- {} -->",
              cp, name,
              char::from_u32(cp)
                .and_then(|c| unicode_names2::name(c))
                .map(|n| n.to_string())
                .unwrap_or_default()
            );
            insertions.push((cmap_range.end, vec![map_line]));
          }
        }
      }
    }

    // Sort insertions by line descending (bottom-to-top)
    insertions.sort_by(|a, b| b.0.cmp(&a.0));

    for (before_line, new_lines) in insertions {
      // Insert before the given line (1-based)
      let idx = before_line.saturating_sub(1);
      for (i, line) in new_lines.into_iter().enumerate() {
        lines.insert(idx + i, line);
      }
    }
  }

  let mut output = lines.join("\n");
  output.push('\n'); // Preserve trailing newline
  std::fs::write(path, &output).context("writing TTX file")?;

  // Update source_lines and rescan line ranges so subsequent saves
  // operate on the correct baseline (fixes bug where new glyphs
  // would be lost on the next save)
  font.source_lines = lines;
  rescan_line_ranges(font);

  Ok(())
}

/// Generate the XML lines for a TTGlyph element.
fn generate_ttglyph_xml(name: &str, contours: &[Contour], bbox: Option<&BBox>) -> Vec<String> {
  let mut out = Vec::new();

  if contours.is_empty() {
    out.push(format!(
      "    <TTGlyph name=\"{name}\"/><!-- contains no outline data -->"
    ));
    return out;
  }

  let bbox = bbox.unwrap();
  out.push(format!(
    "    <TTGlyph name=\"{name}\" xMin=\"{}\" yMin=\"{}\" xMax=\"{}\" yMax=\"{}\">",
    bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max
  ));

  for contour in contours {
    out.push("      <contour>".to_string());
    for pt in &contour.points {
      out.push(format!(
        "        <pt x=\"{}\" y=\"{}\" on=\"1\"/>",
        pt.x, pt.y
      ));
    }
    out.push("      </contour>".to_string());
  }

  out.push("      <instructions><assembly>".to_string());
  out.push("        </assembly></instructions>".to_string());
  out.push("    </TTGlyph>".to_string());

  out
}

/// Replace lines[start-1..=end-1] (1-based inclusive range) with new_lines.
fn replace_lines(lines: &mut Vec<String>, start: usize, end: usize, new_lines: &[String]) {
  if start == 0 || end == 0 || start > lines.len() {
    return;
  }
  let s = start - 1;
  let e = end.min(lines.len());
  lines.splice(s..e, new_lines.iter().cloned());
}

/// Rescan source_lines to rebuild all line ranges after a save.
/// Single pass over all lines using simple string matching.
fn rescan_line_ranges(font: &mut Font) {
  font.glyf_ranges.clear();
  font.glyph_order_range = None;
  font.hmtx_range = None;
  font.glyf_section_end = None;
  font.cmap_format4_range = None;
  font.gsub_lookup5_range = None;

  let mut in_glyf = false;
  let mut in_gsub = false;
  let mut in_lookup5 = false;
  let mut in_cmap4 = false;

  let mut glyph_order_start = 0;
  let mut hmtx_start = 0;
  let mut cmap4_start = 0;
  let mut lookup5_start = 0;
  let mut current_glyph: Option<(String, usize)> = None;

  for (i, line) in font.source_lines.iter().enumerate() {
    let ln = i + 1; // 1-based
    let trimmed = line.trim();

    // GlyphOrder
    if trimmed.starts_with("<GlyphOrder") && !trimmed.starts_with("</") {
      glyph_order_start = ln;
    } else if trimmed.starts_with("</GlyphOrder>") {
      font.glyph_order_range = Some(LineRange { start: glyph_order_start, end: ln });
    }

    // hmtx
    if trimmed.starts_with("<hmtx") && !trimmed.starts_with("</") {
      hmtx_start = ln;
    } else if trimmed.starts_with("</hmtx>") {
      font.hmtx_range = Some(LineRange { start: hmtx_start, end: ln });
    }

    // glyf + TTGlyph ranges
    if trimmed.starts_with("<glyf") && !trimmed.starts_with("</") {
      in_glyf = true;
    } else if trimmed.starts_with("</glyf>") {
      in_glyf = false;
      font.glyf_section_end = Some(ln);
    } else if in_glyf && trimmed.starts_with("<TTGlyph") {
      if let Some(name) = extract_ttglyph_name(trimmed) {
        if trimmed.contains("/>") {
          // Self-closing (empty glyph)
          font.glyf_ranges.insert(name, LineRange { start: ln, end: ln });
        } else {
          current_glyph = Some((name, ln));
        }
      }
    } else if in_glyf && trimmed.starts_with("</TTGlyph>") {
      if let Some((name, start)) = current_glyph.take() {
        font.glyf_ranges.insert(name, LineRange { start, end: ln });
      }
    }

    // cmap_format_4 (platformID=3, platEncID=1)
    if trimmed.starts_with("<cmap_format_4")
      && !trimmed.starts_with("</")
      && trimmed.contains("platformID=\"3\"")
      && trimmed.contains("platEncID=\"1\"")
    {
      in_cmap4 = true;
      cmap4_start = ln;
    } else if trimmed.starts_with("</cmap_format_4>") && in_cmap4 {
      in_cmap4 = false;
      font.cmap_format4_range = Some(LineRange { start: cmap4_start, end: ln });
    }

    // GSUB Lookup 5
    if trimmed.starts_with("<GSUB") && !trimmed.starts_with("</") {
      in_gsub = true;
    } else if trimmed.starts_with("</GSUB>") {
      in_gsub = false;
    }
    if in_gsub && trimmed.starts_with("<Lookup") && trimmed.contains("index=\"5\"") {
      in_lookup5 = true;
      lookup5_start = ln;
    } else if in_lookup5 && trimmed.starts_with("</Lookup>") {
      in_lookup5 = false;
      font.gsub_lookup5_range = Some(LineRange { start: lookup5_start, end: ln });
    }
  }
}

/// Extract the name attribute from a TTGlyph line.
fn extract_ttglyph_name(line: &str) -> Option<String> {
  let idx = line.find("name=\"")? + 6;
  let rest = &line[idx..];
  let end = rest.find('"')?;
  Some(rest[..end].to_string())
}
