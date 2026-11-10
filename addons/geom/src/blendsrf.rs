// SPDX-License-Identifier: LGPL-2.1-or-later
//! Rhino 5 BlendSrf between two surface edges, with Position / Tangency / Curvature
//! continuity at each and a bulge per side.
//!
//! The edges are matched by arc length. At each place the section is a quintic Bézier
//! curve: its ends are the edge points; with Tangency its first derivative continues the
//! isoparametric curve crossing the edge (scaled by the bulge); with Curvature its second
//! derivative does too (the crossing curve reparameterised by the same speed), so the
//! curvature across the edge matches. Stations are refined until the surface is within the
//! tolerance of these sections; the edges themselves are met at every station.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]
use crate::exact::H;
use crate::nurbs::Srf;
use crate::sweep::{Swept, fit_stations};
use om9_addon_abi::session::{Geo, Vec3, add, dist, scale, sub};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Continuity {
    Position,
    Tangency,
    Curvature,
}

/// An edge of a surface: the parameter direction that ends there (0: u, 1: v) and the end.
#[derive(Debug, Clone, Copy)]
pub struct Edge {
    pub dir: usize,
    pub at_end: bool,
}

/// Point, outward first and second derivatives of the crossing isoparametric curve at
/// place `t` (0..1) along the edge.
fn crossing(s: &Srf, e: Edge, t: f64) -> (Vec3, Vec3, Vec3) {
    let at = |x: f64| -> Vec3 {
        // x: distance in from the edge along the crossing parameter (0 at the edge).
        let a = if e.at_end { 1.0 - x } else { x };
        if e.dir == 0 { s.at_st(a, t) } else { s.at_st(t, a) }
    };
    let h = 1e-4;
    let (p0, p1, p2) = (at(0.0), at(h), at(2.0 * h));
    let p3 = at(3.0 * h);
    // One-sided differences (second order), then flip: outward = −inward.
    let d1: Vec3 = std::array::from_fn(|k| -(-3.0 * p0[k] + 4.0 * p1[k] - p2[k]) / (2.0 * h));
    let d2: Vec3 = std::array::from_fn(|k| (2.0 * p0[k] - 5.0 * p1[k] + 4.0 * p2[k] - p3[k]) / (h * h));
    (p0, d1, d2)
}

/// The edge as points along its arc length (for matching the two edges).
struct EdgeArc {
    t: Vec<f64>,
    s: Vec<f64>,
}

impl EdgeArc {
    fn of(srf: &Srf, e: Edge) -> EdgeArc {
        let n = 256;
        let (mut t, mut s) = (vec![0.0], vec![0.0]);
        let mut prev = crossing(srf, e, 0.0).0;
        let mut run = 0.0;
        for i in 1..=n {
            let x = i as f64 / n as f64;
            let p = crossing(srf, e, x).0;
            run += dist(p, prev);
            prev = p;
            t.push(x);
            s.push(run);
        }
        let total = run.max(1e-300);
        s.iter_mut().for_each(|v| *v /= total);
        EdgeArc { t, s }
    }
    fn t_of(&self, s: f64) -> f64 {
        let i = self.s.partition_point(|v| *v <= s).clamp(1, self.s.len() - 1) - 1;
        let f = ((s - self.s[i]) / (self.s[i + 1] - self.s[i]).max(1e-300)).clamp(0.0, 1.0);
        self.t[i] + (self.t[i + 1] - self.t[i]) * f
    }
}

/// Rhino 5 BlendSrf between edge `e1` of `g1` and edge `e2` of `g2`.
pub fn blend(
    g1: &Geo,
    e1: Edge,
    c1: Continuity,
    bulge1: f64,
    g2: &Geo,
    e2: Edge,
    c2: Continuity,
    bulge2: f64,
    tol: f64,
) -> Result<Swept, String> {
    let s1 = Srf::of(g1).ok_or("not a NURBS surface")?;
    let s2 = Srf::of(g2).ok_or("not a NURBS surface")?;
    // Run the second edge the way the first runs (shorter cross connections).
    let (a0, a1) = (crossing(&s1, e1, 0.0).0, crossing(&s1, e1, 1.0).0);
    let (b0, b1) = (crossing(&s2, e2, 0.0).0, crossing(&s2, e2, 1.0).0);
    let flip = dist(a0, b0) + dist(a1, b1) > dist(a0, b1) + dist(a1, b0);
    let (arc1, arc2) = (EdgeArc::of(&s1, e1), EdgeArc::of(&s2, e2));
    let placed = |u: f64| -> Vec<H> {
        let t1 = arc1.t_of(u);
        let t2 = arc2.t_of(if flip { 1.0 - u } else { u });
        let (p, d1p, d2p) = crossing(&s1, e1, t1);
        let (q, d1q, d2q) = crossing(&s2, e2, t2);
        let gap = dist(p, q).max(1e-9);
        // Quintic Bézier: C'(0) = 5(P1 − P0), C''(0) = 20(P2 − 2P1 + P0) (and mirrored at 1).
        // Speed: the crossing curve sped up to λ = bulge·gap/|d1| (so its length scale is the gap).
        let norm = |v: Vec3| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-300);
        let (l1, l2) = (bulge1 * gap / norm(d1p), bulge2 * gap / norm(d1q));
        let (v1, a1) = (scale(d1p, l1), scale(d2p, l1 * l1));
        // At the far end the section arrives going into the second surface: C'(1) = −λ d1q.
        let (v2, a2) = (scale(d1q, -l2), scale(d2q, l2 * l2));
        let p1 = match c1 {
            Continuity::Position => add(p, scale(sub(q, p), 0.2)),
            _ => add(p, scale(v1, 0.2)),
        };
        let p2 = match c1 {
            Continuity::Curvature => add(sub(scale(p1, 2.0), p), scale(a1, 1.0 / 20.0)),
            _ => add(p, scale(sub(q, p), 0.4)),
        };
        let q1 = match c2 {
            Continuity::Position => add(q, scale(sub(p, q), 0.2)),
            _ => sub(q, scale(v2, 0.2)),
        };
        let q2 = match c2 {
            Continuity::Curvature => add(sub(scale(q1, 2.0), q), scale(a2, 1.0 / 20.0)),
            _ => add(q, scale(sub(p, q), 0.4)),
        };
        [p, p1, p2, q2, q1, q].iter().map(|x| [x[0], x[1], x[2], 1.0]).collect()
    };
    fit_stations(&placed, 0.0, 1.0, false, Vec::new(), 5, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0], tol, 400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact::Nc;
    use om9_addon_abi::session::{cross, norm, unit};

    fn plane(o: Vec3, u: Vec3, v: Vec3) -> Geo {
        Geo::Surface {
            degree: [1, 1],
            size: [2, 2],
            points: vec![o, add(o, v), add(o, u), add(add(o, u), v)],
            weights: vec![1.0; 4],
            knots: [vec![0.0, 0.0, 1.0, 1.0], vec![0.0, 0.0, 1.0, 1.0]],
        }
    }

    fn eval(sw: &Swept, u: f64, v: f64) -> Vec3 {
        let row: Vec<H> = (0..sw.rows[0].len())
            .map(|j| Nc { degree: sw.degree[0], points: sw.rows.iter().map(|r| r[j]).collect(), knots: sw.knots[0].clone() }.eval_h(u))
            .collect();
        Nc { degree: sw.degree[1], points: row, knots: sw.knots[1].clone() }.eval(v)
    }

    #[test]
    fn blend_meets_both_edges_tangent() {
        // Two flat strips: z = 0 for x in 0..4 and z = 2 for x in 6..10 (y 0..5); blend the
        // edge x = 4 of the first to the edge x = 6 of the second.
        let a = plane([0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 5.0, 0.0]);
        let b = plane([6.0, 0.0, 2.0], [4.0, 0.0, 0.0], [0.0, 5.0, 0.0]);
        for c in [Continuity::Tangency, Continuity::Curvature] {
            let s = blend(&a, Edge { dir: 0, at_end: true }, c, 1.0, &b, Edge { dir: 0, at_end: false }, c, 1.0, 0.001).unwrap();
            for i in 0..=10 {
                let u = i as f64 / 10.0;
                let (p, q) = (eval(&s, u, 0.0), eval(&s, u, 1.0));
                assert!((p[0] - 4.0).abs() < 1e-9 && p[2].abs() < 1e-9, "{p:?}");
                assert!((q[0] - 6.0).abs() < 1e-9 && (q[2] - 2.0).abs() < 1e-9, "{q:?}");
                // Tangent: leaves flat (no z slope) at both ends.
                let (p2, q2) = (eval(&s, u, 1e-4), eval(&s, u, 1.0 - 1e-4));
                assert!(((p2[2] - p[2]) / (p2[0] - p[0])).abs() < 1e-3, "flat start");
                assert!(((q[2] - q2[2]) / (q[0] - q2[0])).abs() < 1e-3, "flat end");
            }
            let _ = (cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), norm([1.0; 3]), unit([1.0; 3]));
        }
    }
}
