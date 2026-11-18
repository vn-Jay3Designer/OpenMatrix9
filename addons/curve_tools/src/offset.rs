// SPDX-License-Identifier: LGPL-2.1-or-later
//! Offset Curve as Matrix (Rhino) runs it:
//! `Select curve to offset ( Distance=1 Corner=Sharp ThroughPoint=No Tolerance=0.001
//! BothSides=No InCPlane=No Cap=None )`, then `Side to offset`.
//!
//! Lines stay lines, arcs and circles stay concentric arcs and circles, polylines keep straight
//! segments with the chosen corner (Sharp: extended to meet, Round: arcs, Smooth: curvature
//! blends, Chamfer: straight lines; only outside corners), other curves are fitted within
//! `Tolerance`. The offset lies in the curve's plane (`InCPlane=Yes`: the CPlane).
//! `BothSides` offsets to both sides; `Cap=Flat|Round` then closes open results.
use om9_addon_abi::session::{
    Event, Geo, Reply, Vec3, add, cross, dist, dot, norm, scale, sub, unit,
};
use om9_addon_geom::blend::{self, Continuity, End};
use om9_addon_geom::cmd::{Collected, Flow, Show};
use om9_addon_geom::join;
use om9_addon_geom::pick::CurveRef;

/// The plane normal of a curve (the CPlane normal when it is straight or not planar).
fn curve_normal(c: &CurveRef, fallback: Vec3) -> Vec3 {
    if let Some(Geo::Arc { x, y, .. } | Geo::Circle { x, y, .. }) = &c.exact {
        return orient(unit(cross(*x, *y)), fallback);
    }
    let pts: Vec<Vec3> = (0..=64).map(|i| c.at(i as f64 / 64.0)).collect();
    let o = pts[0];
    let mut n = [0.0; 3];
    for w in pts.windows(2) {
        n = add(n, cross(sub(w[0], o), sub(w[1], o)));
    }
    if norm(n) < 1e-9 {
        // Open curves: the turn between chords.
        for w in pts.windows(3) {
            let t = cross(sub(w[1], w[0]), sub(w[2], w[1]));
            n = add(n, if dot(t, n) < 0.0 { scale(t, -1.0) } else { t });
        }
    }
    if norm(n) < 1e-12 {
        return fallback;
    }
    let n = unit(n);
    // Planar within a small tolerance?
    let size = pts
        .iter()
        .map(|p| dist(*p, o))
        .fold(0.0, f64::max)
        .max(1e-9);
    if pts
        .iter()
        .any(|p| dot(sub(*p, o), n).abs() > 1e-6 * size.max(1.0))
    {
        return fallback;
    }
    orient(n, fallback)
}

fn orient(n: Vec3, fallback: Vec3) -> Vec3 {
    if dot(n, fallback) < 0.0 {
        scale(n, -1.0)
    } else {
        n
    }
}

/// Left of the travel direction in the plane with normal `n`.
fn left(n: Vec3, t: Vec3) -> Vec3 {
    unit(cross(n, t))
}

/// +1 when `p` is on the left of the curve, −1 on the right.
fn side_of(c: &CurveRef, n: Vec3, p: Vec3) -> f64 {
    let (t, q) = c.closest(p);
    if dot(sub(p, q), left(n, c.tangent(t))) >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Corner kinds.
#[derive(Clone, Copy, PartialEq)]
enum Corner {
    Sharp,
    Round,
    Smooth,
    Chamfer,
}

/// Offset straight segments (`pts`, closed or open) by `d` to the left (`d < 0`: right).
fn offset_segments(pts: &[Vec3], closed: bool, n: Vec3, d: f64, corner: Corner) -> Option<Geo> {
    let m = pts.len();
    let segs: Vec<(Vec3, Vec3)> = (0..if closed { m } else { m - 1 })
        .map(|i| (pts[i], pts[(i + 1) % m]))
        .filter(|(a, b)| dist(*a, *b) > 1e-12)
        .collect();
    if segs.is_empty() {
        return None;
    }
    let off: Vec<(Vec3, Vec3, Vec3)> = segs
        .iter()
        .map(|(a, b)| {
            let t = unit(sub(*b, *a));
            let o = scale(left(n, t), d);
            (add(*a, o), add(*b, o), t)
        })
        .collect();
    let k = off.len();
    // Ends of each offset segment after the corners are settled.
    let mut starts: Vec<Vec3> = off.iter().map(|s| s.0).collect();
    let mut ends: Vec<Vec3> = off.iter().map(|s| s.1).collect();
    let mut fills: Vec<Option<Geo>> = vec![None; k];
    let corners = if closed { k } else { k - 1 };
    for i in 0..corners {
        let j = (i + 1) % k;
        let (ti, tj) = (off[i].2, off[j].2);
        let turn = dot(cross(ti, tj), n);
        if turn.abs() < 1e-12 && dot(ti, tj) > 0.0 {
            continue; // straight on
        }
        // Outside corner: the offset side is opposite the turn.
        let outside = turn * d < 0.0;
        if !outside || corner == Corner::Sharp {
            // Extend (or trim) both to their crossing.
            let w = cross(ti, tj);
            let ww = dot(w, w);
            if ww < 1e-18 {
                continue;
            }
            let s = dot(cross(sub(off[j].0, off[i].0), tj), w) / ww;
            let x = add(off[i].0, scale(ti, s));
            ends[i] = x;
            starts[j] = x;
            continue;
        }
        let (p, q) = (off[i].1, off[j].0);
        fills[i] = match corner {
            Corner::Chamfer => Some(Geo::Line { start: p, end: q }),
            Corner::Round => blend::arc_from(p, ti, q).or(Some(Geo::Line { start: p, end: q })),
            Corner::Smooth => {
                let zero = [[0.0; 3]; 3];
                let a = End {
                    point: p,
                    d: [ti, zero[0], zero[1], zero[2]],
                };
                let b = End {
                    point: q,
                    d: [scale(tj, -1.0), zero[0], zero[1], zero[2]],
                };
                Some(blend::bezier_blend(
                    &a,
                    Continuity::Curvature,
                    &b,
                    Continuity::Curvature,
                    [1.0, 1.0],
                ))
            }
            Corner::Sharp => None,
        };
    }
    let mut pieces = Vec::new();
    for i in 0..k {
        if dist(starts[i], ends[i]) > 1e-12 {
            pieces.push(Geo::Line {
                start: starts[i],
                end: ends[i],
            });
        }
        if let Some(f) = fills[i].take() {
            pieces.push(f);
        }
    }
    join::join(&pieces, 1e-7)
}

/// Offset one curve by `d` (signed: + left) in the plane with normal `n`.
fn offset_one(c: &CurveRef, n: Vec3, d: f64, corner: Corner, tol: f64) -> Option<Geo> {
    match &c.exact {
        Some(Geo::Line { start, end }) => {
            let o = scale(left(n, unit(sub(*end, *start))), d);
            return Some(Geo::Line {
                start: add(*start, o),
                end: add(*end, o),
            });
        }
        Some(Geo::Arc {
            center,
            x,
            y,
            radius,
            angle,
        }) => {
            // Travel turns about cross(x, y): its left points to the center.
            let inward = dot(unit(cross(*x, *y)), n) > 0.0;
            let r = if inward { radius - d } else { radius + d };
            return (r > 1e-9).then_some(Geo::Arc {
                center: *center,
                x: *x,
                y: *y,
                radius: r,
                angle: *angle,
            });
        }
        Some(Geo::Circle {
            center,
            x,
            y,
            radius,
        }) => {
            let inward = dot(unit(cross(*x, *y)), n) > 0.0;
            let r = if inward { radius - d } else { radius + d };
            return (r > 1e-9).then_some(Geo::Circle {
                center: *center,
                x: *x,
                y: *y,
                radius: r,
            });
        }
        Some(Geo::Polyline { points, closed }) => {
            return offset_segments(points, *closed, n, d, corner);
        }
        Some(Geo::Nurbs {
            degree: 1, points, ..
        }) => {
            let mut p = points.clone();
            if c.closed && p.len() > 2 && dist(p[0], *p.last()?) < 1e-9 {
                p.pop();
            }
            return offset_segments(&p, c.closed, n, d, corner);
        }
        _ => {}
    }
    // Joined curves (lines, arcs and curves with kinks, e.g. a rounded rectangle): each piece
    // offset on its own (lines and arcs exactly), the corners settled, then joined again.
    if let Some(g) = offset_pieces(c, n, d, corner, tol) {
        return Some(g);
    }
    // Smooth curves: offset samples, fit, refine until within tolerance.
    let at = |t: f64| add(c.at(t), scale(left(n, c.tangent(t)), d));
    let mut count = 16;
    loop {
        let m = if c.closed { count } else { count + 1 };
        let pts: Vec<Vec3> = (0..m).map(|i| at(i as f64 / count as f64)).collect();
        let g = om9_addon_geom::interpolate(&pts, 3, c.closed)?;
        let fit = CurveRef {
            exact: Some(g.clone()),
            ..c.clone()
        };
        let worst = (0..count)
            .map(|i| {
                let p = at((i as f64 + 0.5) / count as f64);
                dist(fit.closest(p).1, p)
            })
            .fold(0.0, f64::max);
        if worst <= tol || count >= 1024 {
            return Some(g);
        }
        count *= 2;
    }
}

/// The pieces of an exact NURBS between its kinks (knots of full multiplicity), as normalized
/// parameter ranges.
fn kink_ranges(g: &Geo) -> Vec<(f64, f64)> {
    let Geo::Nurbs { degree, knots, .. } = g else { return Vec::new() };
    let p = *degree;
    if p < 2 || knots.len() < 2 * (p + 1) {
        return Vec::new();
    }
    let (a, b) = (knots[p], knots[knots.len() - p - 1]);
    let mut cuts = vec![0.0];
    let mut i = p + 1;
    while i < knots.len() - p - 1 {
        let mut m = 1;
        while i + m < knots.len() - p - 1 && (knots[i + m] - knots[i]).abs() < 1e-12 {
            m += 1;
        }
        if m >= p {
            cuts.push((knots[i] - a) / (b - a));
        }
        i += m;
    }
    cuts.push(1.0);
    cuts.windows(2).map(|w| (w[0], w[1])).filter(|w| w.1 - w.0 > 1e-9).collect()
}

/// A piece as a line or a circular arc when it is one (within 1e-9 relative), else `None`.
fn as_line_or_arc(g: &Geo) -> Option<Geo> {
    let at = |t: f64| om9_addon_geom::eval_geo(g, t);
    let (a, m, b) = (at(0.0)?, at(0.5)?, at(1.0)?);
    let size = dist(a, b).max(dist(a, m)).max(1e-9);
    let samples: Vec<Vec3> = (0..=16).filter_map(|i| at(i as f64 / 16.0)).collect();
    // Straight.
    let ab = sub(b, a);
    if norm(ab) > 1e-12
        && samples.iter().all(|p| norm(cross(sub(*p, a), unit(ab))) < 1e-9 * size)
    {
        return Some(Geo::Line { start: a, end: b });
    }
    // Circle through start, middle and end.
    let (u, v) = (sub(m, a), sub(b, a));
    let w = cross(u, v);
    let ww = dot(w, w);
    if ww < 1e-24 {
        return None;
    }
    let center = add(a, scale(add(scale(cross(w, u), dot(v, v)), scale(cross(v, w), dot(u, u))), 0.5 / ww));
    let r = dist(center, a);
    if !samples.iter().all(|p| (dist(*p, center) - r).abs() < 1e-9 * size.max(r)) {
        return None;
    }
    let axis = unit(w);
    let x = unit(sub(a, center));
    let y = cross(axis, x);
    let eb = unit(sub(b, center));
    let mut angle = dot(eb, y).atan2(dot(eb, x));
    if angle <= 1e-12 {
        angle += std::f64::consts::TAU;
    }
    Some(Geo::Arc { center, x, y, radius: r, angle })
}

/// Where two offset pieces cross near their shared end (`a` near its end, `b` near its
/// start), as normalized parameters, from polylines of each.
fn crossing(a: &Geo, b: &Geo, n: Vec3) -> Option<(f64, f64)> {
    let k = 400;
    let pa: Vec<Vec3> = (0..=k).filter_map(|i| om9_addon_geom::eval_geo(a, i as f64 / k as f64)).collect();
    let pb: Vec<Vec3> = (0..=k).filter_map(|i| om9_addon_geom::eval_geo(b, i as f64 / k as f64)).collect();
    // In the plane: 2-D coordinates.
    let ex = unit(if norm(cross(n, [1.0, 0.0, 0.0])) > 0.1 { cross(n, [1.0, 0.0, 0.0]) } else { cross(n, [0.0, 1.0, 0.0]) });
    let ey = cross(n, ex);
    let q = |p: Vec3| (dot(p, ex), dot(p, ey));
    let mut best: Option<(f64, f64, f64)> = None;
    for i in 0..k {
        let (p0, p1) = (q(pa[i]), q(pa[i + 1]));
        for j in 0..k {
            let (q0, q1) = (q(pb[j]), q(pb[j + 1]));
            let r = (p1.0 - p0.0, p1.1 - p0.1);
            let s2 = (q1.0 - q0.0, q1.1 - q0.1);
            let den = r.0 * s2.1 - r.1 * s2.0;
            if den.abs() < 1e-18 {
                continue;
            }
            let t = ((q0.0 - p0.0) * s2.1 - (q0.1 - p0.1) * s2.0) / den;
            let u = ((q0.0 - p0.0) * r.1 - (q0.1 - p0.1) * r.0) / den;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                let (ta, tb) = ((i as f64 + t) / k as f64, (j as f64 + u) / k as f64);
                // The crossing nearest the joint: late on `a`, early on `b`.
                let score = (1.0 - ta) + tb;
                if best.is_none_or(|x| score < x.2) {
                    best = Some((ta, tb, score));
                }
            }
        }
    }
    best.map(|b| (b.0, b.1))
}

/// Offset a curve with kinks piece by piece: lines and arcs exactly, other pieces fitted.
/// Corners: two lines meet as `offset_segments` does; a corner with a curved piece is
/// trimmed where the pieces cross (inside) or filled with an arc around the corner
/// (outside; Chamfer: a line). `None` when the curve has no kinks to work with.
fn offset_pieces(c: &CurveRef, n: Vec3, d: f64, corner: Corner, tol: f64) -> Option<Geo> {
    let g = c.exact.as_ref()?;
    let ranges = kink_ranges(g);
    if ranges.len() < 2 {
        return None;
    }
    let pieces: Vec<Geo> = ranges.iter().map(|r| join::sub_geo(g, r.0, r.1)).collect::<Option<_>>()?;
    // Straight pieces only: the polyline offset (corners as asked).
    let shapes: Vec<Option<Geo>> = pieces.iter().map(as_line_or_arc).collect();
    if shapes.iter().all(|s| matches!(s, Some(Geo::Line { .. }))) {
        let mut pts: Vec<Vec3> = shapes
            .iter()
            .filter_map(|s| match s {
                Some(Geo::Line { start, .. }) => Some(*start),
                _ => None,
            })
            .collect();
        if !c.closed {
            if let Some(Some(Geo::Line { end, .. })) = shapes.last() {
                pts.push(*end);
            }
        }
        return offset_segments(&pts, c.closed, n, d, corner);
    }
    let mut out: Vec<Geo> = Vec::new();
    for (piece, shape) in pieces.iter().zip(&shapes) {
        let sub_ref = CurveRef { exact: Some(shape.clone().unwrap_or_else(|| piece.clone())), closed: false, ..c.clone() };
        let o = match shape {
            // An arc that the offset shrinks to nothing (inside a small fillet) drops out; its
            // neighbours then meet.
            Some(Geo::Arc { .. }) => match offset_one(&sub_ref, n, d, corner, tol) {
                Some(o) => o,
                None => continue,
            },
            Some(_) => offset_one(&sub_ref, n, d, corner, tol)?,
            // A free-form piece: fitted on its own (its kinks are the joints, not inside).
            None => {
                let at = |t: f64| add(sub_ref.at(t), scale(left(n, sub_ref.tangent(t)), d));
                let mut count = 8;
                loop {
                    let pts: Vec<Vec3> = (0..=count).map(|i| at(i as f64 / count as f64)).collect();
                    let fit = om9_addon_geom::interpolate(&pts, 3, false)?;
                    let fr = CurveRef { exact: Some(fit.clone()), ..sub_ref.clone() };
                    let worst = (0..count)
                        .map(|i| {
                            let p = at((i as f64 + 0.5) / count as f64);
                            dist(fr.closest(p).1, p)
                        })
                        .fold(0.0, f64::max);
                    if worst <= tol || count >= 512 {
                        break fit;
                    }
                    count *= 2;
                }
            }
        };
        out.push(o);
    }
    // Settle the corners between consecutive offset pieces.
    let k = out.len();
    if k == 0 {
        return None;
    }
    let joints = if c.closed { k } else { k - 1 };
    let mut fills: Vec<Option<Geo>> = vec![None; k];
    for i in 0..joints {
        let j = (i + 1) % k;
        let end_i = om9_addon_geom::eval_geo(&out[i], 1.0)?;
        let start_j = om9_addon_geom::eval_geo(&out[j], 0.0)?;
        if dist(end_i, start_j) <= 1e-9 {
            continue; // tangent joint: the offsets meet
        }
        let both_lines = matches!((&out[i], &out[j]), (Geo::Line { .. }, Geo::Line { .. }));
        if both_lines {
            let (Geo::Line { start: a0, end: a1 }, Geo::Line { start: b0, end: b1 }) = (out[i].clone(), out[j].clone()) else { unreachable!() };
            let (a0, a1, b0, b1) = (&a0, &a1, &b0, &b1);
            let (ti, tj) = (unit(sub(*a1, *a0)), unit(sub(*b1, *b0)));
            let turn = dot(cross(ti, tj), n);
            let outside = turn * d < 0.0;
            if !outside || corner == Corner::Sharp {
                let w = cross(ti, tj);
                let ww = dot(w, w);
                if ww > 1e-18 {
                    let s = dot(cross(sub(*b0, *a0), tj), w) / ww;
                    let x = add(*a0, scale(ti, s));
                    out[i] = Geo::Line { start: *a0, end: x };
                    out[j] = Geo::Line { start: x, end: *b1 };
                    continue;
                }
            }
        }
        // Inside corner: trim both where they cross.
        if let Some((ta, tb)) = crossing(&out[i], &out[j], n) {
            let a = join::sub_geo(&out[i], 0.0, ta)?;
            let b = join::sub_geo(&out[j], tb, 1.0)?;
            out[i] = a;
            out[j] = b;
            continue;
        }
        // Outside corner: an arc round the original corner (a line for Chamfer).
        let t_i = unit(sub(end_i, om9_addon_geom::eval_geo(&out[i], 0.999)?));
        fills[i] = match corner {
            Corner::Chamfer => Some(Geo::Line { start: end_i, end: start_j }),
            _ => blend::arc_from(end_i, t_i, start_j).or(Some(Geo::Line { start: end_i, end: start_j })),
        };
    }
    let mut all = Vec::new();
    for (o, f) in out.into_iter().zip(fills) {
        all.push(o);
        if let Some(f) = f {
            all.push(f);
        }
    }
    join::join(&all, 1e-4)
}

pub(crate) fn offset(e: &Event) -> Reply {
    let flow = Flow::new("Offset")
        .curve("Select curve to offset")
        .point("Side to offset")
        .num("Distance", 1.0)
        .choice("Corner", &["Sharp", "Round", "Smooth", "Chamfer"], "Sharp")
        .toggle("ThroughPoint", false)
        .num("Tolerance", 0.001)
        .toggle("BothSides", false)
        .toggle("InCPlane", false)
        .choice("Cap", &["None", "Flat", "Round"], "None");
    let make = |c: &Collected, side: Vec3| -> Result<Vec<Geo>, String> {
        let (cv, _) = c.curve(0).ok_or("Select curve to offset")?;
        let cplane = e.plane.normal();
        let n = if c.yes(&flow, "InCPlane") {
            cplane
        } else {
            curve_normal(cv, cplane)
        };
        let mut d = c.num(&flow, "Distance");
        if c.yes(&flow, "ThroughPoint") {
            d = dist(cv.closest(side).1, side);
        }
        if d <= 0.0 {
            return Err("Distance must be more than 0".into());
        }
        let tol = match c.num(&flow, "Tolerance") {
            t if t > 0.0 => t,
            _ => 0.001,
        };
        let corner = match c.opt(&flow, "Corner") {
            "Round" => Corner::Round,
            "Smooth" => Corner::Smooth,
            "Chamfer" => Corner::Chamfer,
            _ => Corner::Sharp,
        };
        let s = side_of(cv, n, side);
        if !c.yes(&flow, "BothSides") {
            return offset_one(cv, n, s * d, corner, tol)
                .map(|g| vec![g])
                .ok_or_else(|| "Offset failed (distance too large?)".to_owned());
        }
        let l = offset_one(cv, n, d, corner, tol).ok_or("Offset failed (distance too large?)")?;
        let r = offset_one(cv, n, -d, corner, tol).ok_or("Offset failed (distance too large?)")?;
        let cap = c.opt(&flow, "Cap");
        if cv.closed || cap == "None" {
            return Ok(vec![l, r]);
        }
        let ends = |g: &Geo| {
            (
                om9_addon_geom::eval_geo(g, 0.0),
                om9_addon_geom::eval_geo(g, 1.0),
            )
        };
        let ((Some(l0), Some(l1)), (Some(r0), Some(r1))) = (ends(&l), ends(&r)) else {
            return Ok(vec![l, r]);
        };
        let caps = if cap == "Flat" {
            vec![
                Geo::Line { start: l0, end: r0 },
                Geo::Line { start: l1, end: r1 },
            ]
        } else {
            let t0 = cv.tangent(0.0);
            let t1 = cv.tangent(1.0);
            vec![
                blend::arc_from(l0, scale(t0, -1.0), r0).ok_or("cap")?,
                blend::arc_from(l1, t1, r1).ok_or("cap")?,
            ]
        };
        let mut pieces = vec![l, r];
        pieces.extend(caps);
        Ok(vec![
            join::join(&pieces, 1e-7).ok_or("The cap does not join")?,
        ])
    };
    flow.step(
        e,
        &|c, h| Show {
            geo: match (c.curve(0), h) {
                (Some(_), Some(p)) => make(c, p).unwrap_or_default(),
                _ => Vec::new(),
            },
            rubber: None,
            ..Default::default()
        },
        &|c| {
            let geo = make(c, c.p(0).ok_or("Side to offset")?)?;
            let n = geo.len();
            Ok(Reply::finish(geo).say(format!("Offset: {n} curves")))
        },
    )
}
