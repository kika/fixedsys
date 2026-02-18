use crate::font::{BBox, Contour, PixelGrid, Point};

/// Rasterize contours to a pixel grid using winding number test.
/// Font coordinates: x = col * 10, y = ascent - row * 10 (ascent=130 for this font).
pub fn rasterize(contours: &[Contour], width: usize, height: usize, ascent: i32) -> PixelGrid {
  let mut grid = PixelGrid::new(width, height);
  for row in 0..height {
    for col in 0..width {
      // Sample at pixel center
      let sx = col as i32 * 10 + 5;
      let sy = ascent - row as i32 * 10 - 5;
      if winding_number(contours, sx, sy) != 0 {
        grid.set(col, row, true);
      }
    }
  }
  grid
}

/// Compute winding number of a point relative to contours.
/// Uses horizontal ray casting to the right.
fn winding_number(contours: &[Contour], px: i32, py: i32) -> i32 {
  let mut winding = 0;
  for contour in contours {
    let pts = &contour.points;
    let n = pts.len();
    if n < 2 {
      continue;
    }
    for i in 0..n {
      let p1 = pts[i];
      let p2 = pts[(i + 1) % n];
      // Only consider vertical edges (same x) that cross the horizontal ray
      if p1.x == p2.x {
        // Vertical edge at x = p1.x
        if p1.x <= px {
          continue; // Edge is to the left, doesn't cross rightward ray
        }
        let (y_lo, y_hi) = if p1.y < p2.y {
          (p1.y, p2.y)
        } else {
          (p2.y, p1.y)
        };
        if py >= y_lo && py < y_hi {
          // Edge crosses the ray
          if p1.y < p2.y {
            winding += 1; // Upward edge
          } else {
            winding -= 1; // Downward edge
          }
        }
      }
      // Horizontal edges don't contribute to winding number
    }
  }
  winding
}

/// Vectorize a pixel grid back to contours.
/// Traces boundaries between filled and empty pixels, producing clean
/// rectilinear contours with corners only (no collinear intermediate points).
pub fn vectorize(grid: &PixelGrid, ascent: i32) -> (Vec<Contour>, Option<BBox>) {
  if grid.data.iter().all(|&p| !p) {
    return (Vec::new(), None);
  }

  // Build a set of directed boundary edges.
  // Each edge is between two grid cells. We use the convention:
  // - Horizontal edge at (col, row): bottom of row, from col to col+1
  // - Vertical edge at (col, row): left of col, from row to row+1
  //
  // We store directed edges: (start_point, end_point) in grid coordinates.
  // Grid coordinates: (col, row) where each unit = one pixel = 10 font units.

  let w = grid.width;
  let h = grid.height;

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  struct GridPt(i32, i32); // (col, row) in grid coords (corners, not centers)

  let mut edges: Vec<(GridPt, GridPt)> = Vec::new();

  // Check each boundary between cells.
  // Convention: CW in screen coords (y-down). For a filled pixel at (c,r):
  //   top:    (c,r)→(c+1,r)     rightward
  //   right:  (c+1,r)→(c+1,r+1) downward
  //   bottom: (c+1,r+1)→(c,r+1) leftward
  //   left:   (c,r+1)→(c,r)     upward
  for row in 0..h {
    for col in 0..w {
      let filled = grid.get(col, row);
      let c = col as i32;
      let r = row as i32;

      // Right neighbor
      let right_filled = if col + 1 < w { grid.get(col + 1, row) } else { false };
      if filled && !right_filled {
        // Right boundary: downward
        edges.push((GridPt(c + 1, r), GridPt(c + 1, r + 1)));
      } else if !filled && right_filled {
        // Left boundary of right pixel: upward
        edges.push((GridPt(c + 1, r + 1), GridPt(c + 1, r)));
      }

      // Bottom neighbor
      let bottom_filled = if row + 1 < h { grid.get(col, row + 1) } else { false };
      if filled && !bottom_filled {
        // Bottom boundary: leftward
        edges.push((GridPt(c + 1, r + 1), GridPt(c, r + 1)));
      } else if !filled && bottom_filled {
        // Top boundary of bottom pixel: rightward
        edges.push((GridPt(c, r + 1), GridPt(c + 1, r + 1)));
      }

      // Grid boundary edges (only for filled pixels at edges)
      if row == 0 && filled {
        // Top boundary: rightward
        edges.push((GridPt(c, 0), GridPt(c + 1, 0)));
      }
      if col == 0 && filled {
        // Left boundary: upward
        edges.push((GridPt(0, r + 1), GridPt(0, r)));
      }
      if row == h - 1 && filled {
        // Bottom boundary: leftward
        edges.push((GridPt(c + 1, h as i32), GridPt(c, h as i32)));
      }
      if col == w - 1 && filled {
        // Right boundary: downward
        edges.push((GridPt(w as i32, r), GridPt(w as i32, r + 1)));
      }
    }
  }

  // Build adjacency: from each start point, which edges leave it?
  use std::collections::HashMap;
  let mut adj: HashMap<GridPt, Vec<usize>> = HashMap::new();
  for (i, &(start, _)) in edges.iter().enumerate() {
    adj.entry(start).or_default().push(i);
  }

  let mut used = vec![false; edges.len()];
  let mut contours = Vec::new();

  // Chain edges into closed loops
  for start_idx in 0..edges.len() {
    if used[start_idx] {
      continue;
    }
    let mut loop_pts = Vec::new();
    let mut current_idx = start_idx;

    loop {
      if used[current_idx] {
        break;
      }
      used[current_idx] = true;
      let (_, end) = edges[current_idx];
      loop_pts.push(end);

      // Find next unused edge starting from `end`
      let mut found = false;
      if let Some(candidates) = adj.get(&end) {
        for &ci in candidates {
          if !used[ci] {
            current_idx = ci;
            found = true;
            break;
          }
        }
      }
      if !found {
        break;
      }
    }

    if loop_pts.len() >= 3 {
      // Simplify: remove collinear intermediate points
      let mut simplified = Vec::new();
      let n = loop_pts.len();
      for i in 0..n {
        let prev = loop_pts[(i + n - 1) % n];
        let curr = loop_pts[i];
        let next = loop_pts[(i + 1) % n];
        // Keep point if direction changes (it's a corner)
        let dx1 = curr.0 - prev.0;
        let dy1 = curr.1 - prev.1;
        let dx2 = next.0 - curr.0;
        let dy2 = next.1 - curr.1;
        if dx1 != dx2 || dy1 != dy2 {
          simplified.push(curr);
        }
      }

      if simplified.len() >= 3 {
        // Convert grid coordinates to font coordinates
        let font_pts: Vec<Point> = simplified
          .iter()
          .map(|gp| Point {
            x: gp.0 * 10,
            y: ascent - gp.1 * 10,
          })
          .collect();
        contours.push(Contour { points: font_pts });
      }
    }
  }

  // Compute bounding box
  let mut x_min = i32::MAX;
  let mut y_min = i32::MAX;
  let mut x_max = i32::MIN;
  let mut y_max = i32::MIN;
  for c in &contours {
    for p in &c.points {
      x_min = x_min.min(p.x);
      y_min = y_min.min(p.y);
      x_max = x_max.max(p.x);
      y_max = y_max.max(p.y);
    }
  }
  let bbox = if x_min <= x_max {
    Some(BBox { x_min, y_min, x_max, y_max })
  } else {
    None
  };

  (contours, bbox)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_single_pixel_roundtrip() {
    let mut grid = PixelGrid::new(8, 16);
    grid.set(3, 5, true);
    let (contours, bbox) = vectorize(&grid, 130);
    assert!(!contours.is_empty());
    let bbox = bbox.unwrap();
    assert_eq!(bbox.x_min, 30);
    assert_eq!(bbox.x_max, 40);
    assert_eq!(bbox.y_min, 70);  // bottom of pixel: 130 - 6*10
    assert_eq!(bbox.y_max, 80);  // top of pixel: 130 - 5*10

    let grid2 = rasterize(&contours, 8, 16, 130);
    assert!(grid2.get(3, 5));
    for r in 0..16 {
      for c in 0..8 {
        if r == 5 && c == 3 { continue; }
        assert!(!grid2.get(c, r), "pixel ({c},{r}) should be empty");
      }
    }
  }

  #[test]
  fn test_block_roundtrip() {
    // 3x2 block of pixels
    let mut grid = PixelGrid::new(8, 16);
    for r in 4..6 {
      for c in 2..5 {
        grid.set(c, r, true);
      }
    }
    let (contours, _) = vectorize(&grid, 130);
    let grid2 = rasterize(&contours, 8, 16, 130);
    for r in 0..16 {
      for c in 0..8 {
        let expected = r >= 4 && r < 6 && c >= 2 && c < 5;
        assert_eq!(grid2.get(c, r), expected, "pixel ({c},{r})");
      }
    }
  }

  #[test]
  fn test_letter_a_roundtrip() {
    // Reconstruct the A glyph from the font
    let mut grid = PixelGrid::new(8, 16);
    // Row 4: cols 3,4 (top of A)
    grid.set(3, 4, true); grid.set(4, 4, true);
    // Row 5: cols 2,3,4,5
    grid.set(2, 5, true); grid.set(3, 5, true);
    grid.set(4, 5, true); grid.set(5, 5, true);
    // Row 6-8: cols 1,2 and 5,6 (sides)
    for r in 6..9 {
      grid.set(1, r, true); grid.set(2, r, true);
      grid.set(5, r, true); grid.set(6, r, true);
    }
    // Row 9: cols 1-6 (crossbar)
    for c in 1..7 {
      grid.set(c, 9, true);
    }
    // Row 10-12: cols 1,2 and 5,6 (sides)
    for r in 10..13 {
      grid.set(1, r, true); grid.set(2, r, true);
      grid.set(5, r, true); grid.set(6, r, true);
    }

    let (contours, _) = vectorize(&grid, 130);
    let grid2 = rasterize(&contours, 8, 16, 130);
    for r in 0..16 {
      for c in 0..8 {
        assert_eq!(grid.get(c, r), grid2.get(c, r),
          "pixel ({c},{r}): expected={}, got={}", grid.get(c, r), grid2.get(c, r));
      }
    }
  }
}
