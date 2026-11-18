// SPDX-License-Identifier: LGPL-2.1-or-later
//! Light previews while the pointer moves (Hover), exact geometry only when the command
//! finishes (Enter / last click).
//!
//! A preview step has to stay well under one frame (16 ms) even with thousands of objects
//! selected, so it never rebuilds full geometry:
//! - rigid and affine commands (Move, Rotate, Scale, Mirror, Array …) answer only matrices
//!   (`Reply::preview_transforms`, checked with [`is_valid_preview_matrix`]); the host draws
//!   the selection under them;
//! - deformations (Bend, Twist, Taper, Stretch, Flow, Cage …) draw [`preview_deformed_selection`]:
//!   each curve with at most [`HOVER_CURVE_SEGMENTS`] segments and every other object as its
//!   bounding box with each edge cut in [`BOX_EDGE_SEGMENTS`] (so a bend shows), for at most
//!   [`MAX_HOVER_WIRE_OBJECTS`] objects and [`MAX_HOVER_WIRE_SEGMENTS`] segments, evenly sampled
//!   from the selection. A small selection
//!   (at most [`EXACT_PREVIEW_OBJECTS`] objects and [`EXACT_PREVIEW_TRIANGLES`] triangles)
//!   still previews its real deformed shape.
use crate::cmd::deformed;
use om9_addon_abi::session::{Geo, Selected, Vec3, add, dist, scale, sub};

/// Pointer moves shorter than this (mm) change nothing worth recomputing.
pub const HOVER_EPSILON_MM: f64 = 1e-4;
/// Segments of a curve in a pointer-move preview.
pub const HOVER_CURVE_SEGMENTS: usize = 16;
/// Pieces of each bounding box edge in a deformed preview.
pub const BOX_EDGE_SEGMENTS: usize = 8;
/// Objects drawn at most in a pointer-move preview.
pub const MAX_HOVER_WIRE_OBJECTS: usize = 256;
/// Segments drawn at most in a deformed wire preview (every viewport draws them each frame).
pub const MAX_HOVER_WIRE_SEGMENTS: usize = 8_000;
/// A selection this small previews its real deformed shape.
pub const EXACT_PREVIEW_OBJECTS: usize = 16;
/// … when its meshes have at most this many triangles in all.
pub const EXACT_PREVIEW_TRIANGLES: usize = 5_000;

/// Whether the pointer moved enough to preview again.
pub fn hover_changed(prev: Option<Vec3>, current: Vec3) -> bool {
    prev.is_none_or(|p| dist(p, current) >= HOVER_EPSILON_MM)
}

/// A 4×4 row-major matrix the host can draw: finite, last row (0, 0, 0, 1) and its 3×3 part
/// not singular.
pub fn is_valid_preview_matrix(m: &[f64; 16]) -> bool {
    if !m.iter().all(|v| v.is_finite()) || m[12..16] != [0.0, 0.0, 0.0, 1.0] {
        return false;
    }
    let a = |r: usize, c: usize| m[r * 4 + c];
    let det = a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
        + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0));
    det.abs() > 1e-12
}

/// At most `segments` segments along a polyline, by length; a single segment is cut into
/// `segments` pieces so a bend or twist shows on it.
pub fn sample_polyline_coarse(points: &[Vec3], segments: usize) -> Vec<Vec3> {
    let segments = segments.max(1);
    if points.len() < 2 {
        return points.to_vec();
    }
    let lengths: Vec<f64> = points.windows(2).map(|w| dist(w[0], w[1])).collect();
    let total: f64 = lengths.iter().sum();
    if total <= 0.0 {
        return vec![points[0]];
    }
    if points.len() - 1 <= segments && points.len() > 2 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(segments + 1);
    let (mut i, mut walked) = (0usize, 0.0);
    for k in 0..=segments {
        let target = total * k as f64 / segments as f64;
        while i + 1 < lengths.len() && walked + lengths[i] < target {
            walked += lengths[i];
            i += 1;
        }
        let t = if lengths[i] > 0.0 { ((target - walked) / lengths[i]).clamp(0.0, 1.0) } else { 0.0 };
        out.push(add(points[i], scale(sub(points[i + 1], points[i]), t)));
    }
    out
}

/// The 12 edges of a bounding box, each cut in [`BOX_EDGE_SEGMENTS`] pieces, deformed by `f`.
pub fn deformed_bbox_wire(bbox: [Vec3; 2], f: &dyn Fn(Vec3) -> Vec3) -> Vec<Geo> {
    let [lo, hi] = bbox;
    if !lo.iter().chain(hi.iter()).all(|v| v.is_finite()) {
        return Vec::new();
    }
    let c = |i: usize| -> Vec3 {
        [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]
    };
    const EDGES: [(usize, usize); 12] = [(0, 1), (1, 3), (3, 2), (2, 0), (4, 5), (5, 7), (7, 6), (6, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    // A flat box (zero height …) has its edges twice: each drawn once.
    let mut seen: Vec<(Vec3, Vec3)> = Vec::new();
    EDGES
        .iter()
        .filter(|(a, b)| {
            let (p, q) = (c(*a), c(*b));
            let dup = seen.iter().any(|(x, y)| (*x == p && *y == q) || (*x == q && *y == p));
            seen.push((p, q));
            dist(p, q) > 0.0 && !dup
        })
        .map(|(a, b)| {
            let (p, q) = (c(*a), c(*b));
            let points = (0..=BOX_EDGE_SEGMENTS)
                .map(|i| f(add(p, scale(sub(q, p), i as f64 / BOX_EDGE_SEGMENTS as f64))))
                .collect();
            Geo::Polyline { points, closed: false }
        })
        .collect()
}

/// Indices of at most `n` items out of `len`, evenly spread (all of them when they fit).
fn spread(len: usize, n: usize) -> impl Iterator<Item = usize> {
    let step = if len <= n { 1.0 } else { len as f64 / n as f64 };
    let count = len.min(n);
    (0..count).map(move |i| ((i as f64 * step) as usize).min(len.saturating_sub(1)))
}

/// A light preview of the selection deformed by `f` (see the module notes).
pub fn preview_deformed_selection(selection: &[Selected], f: &dyn Fn(Vec3) -> Vec3) -> Vec<Geo> {
    let triangles: usize = selection.iter().filter_map(|s| s.mesh.as_ref()).map(|m| m.triangles.len()).sum();
    if !selection.is_empty() && selection.len() <= EXACT_PREVIEW_OBJECTS && triangles <= EXACT_PREVIEW_TRIANGLES {
        let exact: Vec<Geo> = selection.iter().filter_map(|s| deformed(s, f)).collect();
        if exact.len() == selection.len() {
            return exact;
        }
    }
    // Boxes cost 12 × BOX_EDGE_SEGMENTS segments each, curves HOVER_CURVE_SEGMENTS: as many
    // objects as fit the segment budget, at most MAX_HOVER_WIRE_OBJECTS.
    let per_object = 12 * BOX_EDGE_SEGMENTS;
    let objects = (MAX_HOVER_WIRE_SEGMENTS / per_object).clamp(1, MAX_HOVER_WIRE_OBJECTS);
    let mut out = Vec::new();
    for i in spread(selection.len(), objects) {
        let s = &selection[i];
        if s.kind == "curve" && s.points.len() >= 2 {
            let mut pts = s.points.clone();
            if s.closed {
                pts.push(pts[0]);
            }
            let coarse = sample_polyline_coarse(&pts, HOVER_CURVE_SEGMENTS);
            out.push(Geo::Polyline { points: coarse.into_iter().map(f).collect(), closed: false });
        } else if s.kind == "point" && !s.points.is_empty() {
            out.push(Geo::Point { point: f(s.points[0]) });
        } else {
            out.extend(deformed_bbox_wire(s.bbox, f));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(id: &str, kind: &str, points: Vec<Vec3>, bbox: [Vec3; 2]) -> Selected {
        Selected {
            id: id.into(),
            kind: kind.into(),
            name: String::new(),
            bbox,
            points,
            closed: false,
            gem: None,
            mesh: None,
            exact: None,
            sub: Vec::new(),
        }
    }

    #[test]
    fn hover_and_matrices() {
        assert!(hover_changed(None, [0.0; 3]));
        assert!(!hover_changed(Some([0.0; 3]), [5e-5, 0.0, 0.0]));
        assert!(hover_changed(Some([0.0; 3]), [2e-4, 0.0, 0.0]));
        let id = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        assert!(is_valid_preview_matrix(&id));
        let mut flat = id;
        flat[10] = 0.0;
        assert!(!is_valid_preview_matrix(&flat), "singular");
        let mut nan = id;
        nan[3] = f64::NAN;
        assert!(!is_valid_preview_matrix(&nan));
        let mut proj = id;
        proj[12] = 1.0;
        assert!(!is_valid_preview_matrix(&proj), "not affine");
    }

    #[test]
    fn coarse_curves() {
        // A line is cut into 16 so a bend shows; a dense curve is reduced to 16 by length.
        let line = sample_polyline_coarse(&[[0.0; 3], [16.0, 0.0, 0.0]], 16);
        assert_eq!(line.len(), 17);
        assert!(dist(line[3], [3.0, 0.0, 0.0]) < 1e-12);
        let dense: Vec<Vec3> = (0..=1000).map(|i| [i as f64 * 0.01, (i as f64 * 0.01).sin(), 0.0]).collect();
        let c = sample_polyline_coarse(&dense, 16);
        assert_eq!(c.len(), 17);
        assert!(dist(c[0], dense[0]) < 1e-12 && dist(c[16], dense[1000]) < 1e-12, "ends kept");
        // Short polylines stay as they are; degenerate input does not panic.
        assert_eq!(sample_polyline_coarse(&[[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]], 16).len(), 3);
        assert_eq!(sample_polyline_coarse(&[[1.0; 3], [1.0; 3]], 16).len(), 1);
        assert!(sample_polyline_coarse(&[], 16).is_empty());
    }

    #[test]
    fn deformed_wires_are_bounded() {
        let bend = |p: Vec3| [p[0], p[1] + 0.1 * p[0] * p[0], p[2]];
        let wire = deformed_bbox_wire([[0.0; 3], [2.0, 1.0, 1.0]], &bend);
        assert_eq!(wire.len(), 12);
        let Geo::Polyline { points, .. } = &wire[0] else { panic!() };
        assert_eq!(points.len(), BOX_EDGE_SEGMENTS + 1);
        assert!(deformed_bbox_wire([[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]], &bend).is_empty(), "no box: nothing");
        // A flat box (a curve lying in a plane): only its real edges.
        assert_eq!(deformed_bbox_wire([[0.0; 3], [1.0, 1.0, 0.0]], &bend).len(), 4);
        // 20 000 stones: at most 256 boxes drawn (12 wires each), whatever the selection size.
        let stones: Vec<Selected> = (0..20_000)
            .map(|i| {
                let o = [(i % 200) as f64, (i / 200) as f64, 0.0];
                sel(&i.to_string(), "mesh", Vec::new(), [o, add(o, [0.5, 0.5, 0.3])])
            })
            .collect();
        let t = std::time::Instant::now();
        let p = preview_deformed_selection(&stones, &bend);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let drawn = (MAX_HOVER_WIRE_SEGMENTS / (12 * BOX_EDGE_SEGMENTS)).min(MAX_HOVER_WIRE_OBJECTS);
        assert_eq!(p.len(), drawn * 12);
        let segments: usize = p.iter().map(|g| match g { Geo::Polyline { points, .. } => points.len() - 1, _ => 0 }).sum();
        assert!(segments <= MAX_HOVER_WIRE_SEGMENTS, "{segments} segments");
        assert!(ms < 50.0, "{ms} ms (debug build)");
        // Spread over the whole selection, not only the first ones.
        let Geo::Polyline { points, .. } = &p[p.len() - 1] else { panic!() };
        assert!(points[0][1] > 90.0, "the last boxes come from the end of the selection: {:?}", points[0]);
        // Curves: coarse.
        let crv = sel("c", "curve", (0..=500).map(|i| [i as f64 * 0.02, 0.0, 0.0]).collect(), [[0.0; 3], [10.0, 0.0, 0.0]]);
        let p = preview_deformed_selection(&[crv.clone(), crv.clone(), crv], &bend);
        // Small selection without meshes: exact deformed curves (no `exact` here: from points).
        assert_eq!(p.len(), 3);
        assert!(preview_deformed_selection(&[], &bend).is_empty());
    }
}
