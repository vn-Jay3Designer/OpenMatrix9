// SPDX-License-Identifier: LGPL-2.1-or-later
//! Joined curves for add-ons: a polycurve (lines, arcs and NURBS end to end) as one exact
//! NURBS curve with a kink at each joint, so an add-on keeps the corners and the exact arcs
//! instead of a dense sample polyline.
//!
//! Every piece becomes a clamped NURBS of the highest degree among the pieces: a line or an arc
//! as Bézier spans (arcs as rational quadratics of at most 90°) raised to that degree, a NURBS
//! of that degree as it is. The pieces are then laid end to end with full-multiplicity knots
//! at the joints (one parameter unit per piece). Only lines become a polyline.
use om9_addon_abi::session::{self as s, Geo, Vec3};

type H = [f64; 4];

fn h(p: Vec3, w: f64) -> H {
    [p[0] * w, p[1] * w, p[2] * w, w]
}

/// One clamped NURBS piece in homogeneous form: degree, poles, knots.
struct Piece {
    degree: usize,
    poles: Vec<H>,
    knots: Vec<f64>,
}

/// Bézier spans (homogeneous) of an arc of `angle` radians, each at most 90°.
fn arc_spans(center: Vec3, x: Vec3, y: Vec3, radius: f64, angle: f64) -> Vec<Vec<H>> {
    let n = ((angle.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
    let step = angle / n as f64;
    let at = |a: f64| s::add(center, s::add(s::scale(x, radius * a.cos()), s::scale(y, radius * a.sin())));
    (0..n)
        .map(|i| {
            let (a0, a1) = (step * i as f64, step * (i + 1) as f64);
            let mid = (a0 + a1) / 2.0;
            let w = (step / 2.0).cos();
            let corner = s::add(center, s::add(s::scale(x, radius / w * mid.cos()), s::scale(y, radius / w * mid.sin())));
            vec![h(at(a0), 1.0), h(corner, w), h(at(a1), 1.0)]
        })
        .collect()
}

/// A Bézier span raised by one degree (homogeneous control points).
fn elevate(b: &[H]) -> Vec<H> {
    let p = b.len() - 1;
    let mut out = vec![b[0]];
    for i in 1..=p {
        let a = i as f64 / (p + 1) as f64;
        out.push(std::array::from_fn(|k| a * b[i - 1][k] + (1.0 - a) * b[i][k]));
    }
    out.push(b[p]);
    out
}

/// Bézier spans laid end to end as a clamped NURBS piece of degree `q`.
fn from_spans(spans: Vec<Vec<H>>, q: usize) -> Piece {
    let mut poles: Vec<H> = Vec::new();
    let mut knots = vec![0.0; q + 1];
    let n = spans.len();
    for (k, mut b) in spans.into_iter().enumerate() {
        while b.len() - 1 < q {
            b = elevate(&b);
        }
        if k == 0 {
            poles.push(b[0]);
        }
        poles.extend_from_slice(&b[1..]);
        if k + 1 < n {
            knots.extend(std::iter::repeat_n((k + 1) as f64 / n as f64, q));
        }
    }
    knots.extend(std::iter::repeat_n(1.0, q + 1));
    Piece { degree: q, poles, knots }
}

/// The natural degree of a piece (1 line or polyline, 2 arc, NURBS its own).
fn degree_of(g: &Geo) -> Option<usize> {
    Some(match g {
        Geo::Line { .. } | Geo::Polyline { .. } => 1,
        Geo::Arc { .. } | Geo::Circle { .. } => 2,
        Geo::Nurbs { degree, .. } => *degree,
        _ => return None,
    })
}

/// A piece at degree `q`, or `None` (a NURBS of lower degree than the others, a NURBS that is
/// not clamped, or a shape that is not a curve).
fn piece(g: &Geo, q: usize) -> Option<Piece> {
    Some(match g {
        Geo::Line { start, end } => from_spans(vec![vec![h(*start, 1.0), h(*end, 1.0)]], q),
        Geo::Polyline { points, closed } => {
            let mut p = points.clone();
            if *closed && p.len() > 2 {
                p.push(p[0]);
            }
            from_spans(p.windows(2).map(|w| vec![h(w[0], 1.0), h(w[1], 1.0)]).collect(), q)
        }
        Geo::Arc { center, x, y, radius, angle } => from_spans(arc_spans(*center, *x, *y, *radius, *angle), q),
        Geo::Circle { center, x, y, radius } => from_spans(arc_spans(*center, *x, *y, *radius, std::f64::consts::TAU), q),
        Geo::Nurbs { degree, points, weights, knots } => {
            if *degree != q || knots.len() != points.len() + degree + 1 {
                return None;
            }
            let (a, b) = (knots[*degree], knots[knots.len() - degree - 1]);
            // Clamped ends only (a periodic curve would need unwrapping first).
            let clamped = knots[..=*degree].iter().all(|k| (k - a).abs() < 1e-12)
                && knots[knots.len() - degree - 1..].iter().all(|k| (k - b).abs() < 1e-12);
            if !clamped || b - a <= 0.0 {
                return None;
            }
            let w = |i: usize| weights.get(i).copied().unwrap_or(1.0);
            Piece {
                degree: q,
                poles: points.iter().enumerate().map(|(i, p)| h(*p, w(i))).collect(),
                knots: knots.iter().map(|k| (k - a) / (b - a)).collect(),
            }
        }
        _ => return None,
    })
}

fn euclid(p: H) -> Vec3 {
    [p[0] / p[3], p[1] / p[3], p[2] / p[3]]
}

/// The pieces of a polycurve (world mm, in order, each running from the previous end) as one
/// curve: a polyline when every piece is straight, else one NURBS of the highest degree.
/// `None` when a piece cannot be represented exactly that way (the add-on then gets samples).
pub fn joined(parts: &[Geo]) -> Option<Geo> {
    if parts.is_empty() {
        return None;
    }
    if parts.len() == 1 {
        return Some(parts[0].clone());
    }
    let q = parts.iter().map(degree_of).collect::<Option<Vec<_>>>()?.into_iter().max()?;
    let pieces: Vec<Piece> = parts.iter().map(|g| piece(g, q)).collect::<Option<_>>()?;
    let first = euclid(pieces[0].poles[0]);
    let last = euclid(*pieces.last()?.poles.last()?);
    let closed = s::dist(first, last) < 1e-6;
    if q == 1 {
        let mut points: Vec<Vec3> = vec![first];
        for p in &pieces {
            points.extend(p.poles[1..].iter().map(|x| euclid(*x)));
        }
        if closed {
            points.pop();
        }
        return Some(Geo::Polyline { points, closed });
    }
    let mut poles: Vec<H> = vec![pieces[0].poles[0]];
    let mut knots = vec![0.0; q + 1];
    for (k, p) in pieces.iter().enumerate() {
        // The joint pole is shared: the next piece's first pole is this piece's last (scaled to
        // the same weight so the curve stays continuous).
        let scale = poles.last()?[3] / p.poles[0][3];
        poles.extend(p.poles[1..].iter().map(|x| x.map(|v| v * scale)));
        let base = k as f64;
        let inner = &p.knots[q + 1..p.knots.len() - q - 1];
        knots.extend(inner.iter().map(|t| base + t));
        if k + 1 < pieces.len() {
            knots.extend(std::iter::repeat_n(base + 1.0, q));
        }
    }
    knots.extend(std::iter::repeat_n(pieces.len() as f64, q + 1));
    let n = pieces.len() as f64;
    let rational = poles.iter().any(|p| (p[3] - 1.0).abs() > 1e-12);
    Some(Geo::Nurbs {
        degree: q,
        points: poles.iter().map(|p| euclid(*p)).collect(),
        weights: if rational { poles.iter().map(|p| p[3]).collect() } else { Vec::new() },
        knots: knots.into_iter().map(|t| t / n).collect(),
    })
}

/// The same curve run the other way (a reversed OM9 curve or polycurve).
pub fn reversed(g: Geo) -> Geo {
    match g {
        Geo::Line { start, end } => Geo::Line { start: end, end: start },
        Geo::Polyline { mut points, closed } => {
            points.reverse();
            if closed {
                // Still starts where it started.
                points.rotate_right(1);
            }
            Geo::Polyline { points, closed }
        }
        Geo::Arc { center, x, y, radius, angle } => {
            let (sa, ca) = angle.sin_cos();
            Geo::Arc {
                center,
                x: s::add(s::scale(x, ca), s::scale(y, sa)),
                y: s::sub(s::scale(x, sa), s::scale(y, ca)),
                radius,
                angle,
            }
        }
        Geo::Circle { center, x, y, radius } => Geo::Circle { center, x, y: s::scale(y, -1.0), radius },
        Geo::Nurbs { degree, mut points, mut weights, knots } => {
            points.reverse();
            weights.reverse();
            let (a, b) = (knots[0], knots[knots.len() - 1]);
            let knots = knots.iter().rev().map(|k| a + b - k).collect();
            Geo::Nurbs { degree, points, weights, knots }
        }
        other => other,
    }
}
