// SPDX-License-Identifier: LGPL-2.1-or-later
//! Rhino 5 FilletSrf: a constant-radius rolling-ball surface between two surfaces.
//!
//! The ball centre runs along the intersection of the two surfaces offset by the radius
//! towards each other (traced by marching with Newton corrections on the exact surfaces); at
//! each centre the cross-section is the exact circular arc (rational quadratic) between the
//! two contact points. Stations along the centre line are refined until the surface is
//! within the tolerance of the true rolling-ball surface; the contact edges lie on the input
//! surfaces to the same tolerance.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]
use crate::exact::H;
use crate::nurbs::Srf;
use crate::offset::normal;
use crate::sweep::{Swept, fit_stations};
use om9_addon_abi::session::{Geo, Vec3, add, cross, dist, dot, norm, scale, sub, unit};

/// One ball position: parameters on both surfaces (normalised), centre and contacts.
#[derive(Debug, Clone, Copy)]
struct Ball {
    x: [f64; 4],
    c: Vec3,
    p1: Vec3,
    p2: Vec3,
}

struct Pair<'a> {
    s1: Srf<'a>,
    s2: Srf<'a>,
    r: f64,
    /// Which side of each surface the ball rolls on (+1 along its normal, −1 against).
    side: [f64; 2],
    /// ∂u × ∂v against `normal`'s side, per surface (found once).
    orient: [f64; 2],
}

/// A fast unit normal: central differences (one-sided at the edges), `orient` sets the side;
/// falls back to the careful one at singular points.
fn quick_normal(s: &Srf, a: f64, b: f64, orient: f64) -> Vec3 {
    let h = 1e-6;
    let (a0, a1) = ((a - h).max(0.0), (a + h).min(1.0));
    let (b0, b1) = ((b - h).max(0.0), (b + h).min(1.0));
    let du = sub(s.at_st(a1, b), s.at_st(a0, b));
    let dv = sub(s.at_st(a, b1), s.at_st(a, b0));
    let n = cross(du, dv);
    if norm(n) > 1e-18 { scale(unit(n), orient) } else { normal(s, a, b) }
}

fn orientation(s: &Srf) -> f64 {
    let n = quick_normal(s, 0.5, 0.5, 1.0);
    if dot(n, normal(s, 0.5, 0.5)) < 0.0 { -1.0 } else { 1.0 }
}

impl Pair<'_> {
    fn n1(&self, a: f64, b: f64) -> Vec3 {
        quick_normal(&self.s1, a, b, self.orient[0])
    }
    fn n2(&self, a: f64, b: f64) -> Vec3 {
        quick_normal(&self.s2, a, b, self.orient[1])
    }
    fn off1(&self, a: f64, b: f64) -> Vec3 {
        add(self.s1.at_st(a, b), scale(self.n1(a, b), self.side[0] * self.r))
    }
    fn off2(&self, a: f64, b: f64) -> Vec3 {
        add(self.s2.at_st(a, b), scale(self.n2(a, b), self.side[1] * self.r))
    }
    fn ball(&self, x: [f64; 4]) -> Ball {
        let c1 = self.off1(x[0], x[1]);
        let c2 = self.off2(x[2], x[3]);
        Ball { x, c: scale(add(c1, c2), 0.5), p1: self.s1.at_st(x[0], x[1]), p2: self.s2.at_st(x[2], x[3]) }
    }
    /// Residual: the two offset points apart, and (with a plane) the distance from it.
    fn residual(&self, x: [f64; 4], plane: Option<(Vec3, Vec3)>) -> [f64; 4] {
        let d = sub(self.off1(x[0], x[1]), self.off2(x[2], x[3]));
        let e = plane.map_or(0.0, |(o, n)| dot(sub(self.off1(x[0], x[1]), o), n));
        [d[0], d[1], d[2], e]
    }

    /// Newton: the ball where the offsets meet (on the plane, when given). `None` when it
    /// leaves a surface or does not converge.
    fn solve(&self, x: [f64; 4], plane: Option<(Vec3, Vec3)>) -> Option<[f64; 4]> {
        self.solve_fixing(x, plane, None)
    }

    /// As `solve`, with one parameter held where it is (a ball on a surface edge).
    fn solve_fixing(&self, mut x: [f64; 4], plane: Option<(Vec3, Vec3)>, fixed: Option<usize>) -> Option<[f64; 4]> {
        let h = 1e-6;
        let mut last = f64::MAX;
        for it in 0..40 {
            let f = self.residual(x, plane);
            let size = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2] + f[3] * f[3]).sqrt();
            if size < 1e-10 * self.r.max(1.0) {
                return Some(x);
            }
            // Stuck against a surface edge: give up early.
            if it > 3 && size > 0.5 * last && x.iter().any(|v| *v <= 0.0 || *v >= 1.0) {
                return None;
            }
            last = size;
            // Jacobian (numeric).
            let mut j = [[0.0; 4]; 4];
            for k in 0..4 {
                if fixed == Some(k) {
                    continue;
                }
                let mut xs = x;
                let step = if x[k] + h <= 1.0 { h } else { -h };
                xs[k] = x[k] + step;
                let fs = self.residual(xs, plane);
                for r in 0..4 {
                    j[r][k] = (fs[r] - f[r]) / step;
                }
            }
            let rows = if plane.is_some() { 4 } else { 3 };
            // Minimum-norm step: dx = Jᵀ (J Jᵀ)⁻¹ (−f) (square case: J⁻¹ (−f)).
            let mut jjt = vec![vec![0.0; rows]; rows];
            for a in 0..rows {
                for b in 0..rows {
                    jjt[a][b] = (0..4).map(|k| j[a][k] * j[b][k]).sum();
                }
            }
            let y = solve_small(jjt, (0..rows).map(|a| -f[a]).collect())?;
            let mut dx = [0.0; 4];
            for k in 0..4 {
                dx[k] = (0..rows).map(|a| j[a][k] * y[a]).sum();
            }
            // Damp large steps.
            let len = dx.iter().map(|v| v * v).sum::<f64>().sqrt();
            let damp = if len > 0.1 { 0.1 / len } else { 1.0 };
            let mut off = false;
            for k in 0..4 {
                let v = x[k] + dx[k] * damp;
                if !(-1e-9..=1.0 + 1e-9).contains(&v) {
                    off = true;
                }
                x[k] = v.clamp(0.0, 1.0);
            }
            if off && len * damp < 1e-12 {
                return None;
            }
            if len * damp < 1e-14 {
                break;
            }
        }
        let f = self.residual(x, plane);
        ((f[0] * f[0] + f[1] * f[1] + f[2] * f[2] + f[3] * f[3]).sqrt() < 1e-8 * self.r.max(1.0)).then_some(x)
    }

    fn tangent(&self, b: &Ball) -> Vec3 {
        unit(cross(self.n1(b.x[0], b.x[1]), self.n2(b.x[2], b.x[3])))
    }
}

fn self_solve_edge(pair: &Pair, x: [f64; 4], k: usize) -> Option<[f64; 4]> {
    pair.solve_fixing(x, None, Some(k))
}

fn solve_small(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|x, y| a[*x][c].abs().total_cmp(&a[*y][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            for k in c..n {
                a[r][k] -= f * a[c][k];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        x[r] = (b[r] - (r + 1..n).map(|k| a[r][k] * x[k]).sum::<f64>()) / a[r][r];
    }
    Some(x)
}

/// Rational quadratic arc from `p1` to `p2` round `c` (shorter way, under 180°): 3 poles.
fn arc(c: Vec3, p1: Vec3, p2: Vec3) -> Vec<H> {
    let (a, b) = (sub(p1, c), sub(p2, c));
    let r = norm(a);
    let cosang = (dot(a, b) / (r * norm(b)).max(1e-300)).clamp(-1.0, 1.0);
    let half = cosang.acos() / 2.0;
    let w = half.cos().max(1e-6);
    let m = unit(add(unit(a), unit(b)));
    let mid = add(c, scale(m, r / w));
    [[p1[0], p1[1], p1[2], 1.0], [mid[0] * w, mid[1] * w, mid[2] * w, w], [p2[0], p2[1], p2[2], 1.0]].to_vec()
}

/// Trace the ball centres (with r = 0: the intersection) from end to end: the balls and their
/// arc-length places along the centre line (0..1).
fn trace(pair: &Pair, size: f64) -> Result<(Vec<Ball>, Vec<f64>), String> {
    // Start: the closest offset points on a grid, then Newton.
    let k = 16;
    let g1p: Vec<(f64, f64, Vec3)> = (0..=k)
        .flat_map(|i| (0..=k).map(move |j| (i as f64 / k as f64, j as f64 / k as f64)))
        .map(|(a, b)| (a, b, pair.off1(a, b)))
        .collect();
    let g2p: Vec<(f64, f64, Vec3)> = (0..=k)
        .flat_map(|i| (0..=k).map(move |j| (i as f64 / k as f64, j as f64 / k as f64)))
        .map(|(a, b)| (a, b, pair.off2(a, b)))
        .collect();
    let mut seeds: Vec<(f64, [f64; 4])> = Vec::new();
    for p in &g1p {
        for q in &g2p {
            seeds.push((dist(p.2, q.2), [p.0, p.1, q.0, q.1]));
        }
    }
    let keep = 12.min(seeds.len());
    if seeds.len() > keep {
        seeds.select_nth_unstable_by(keep - 1, |a, b| a.0.total_cmp(&b.0));
        seeds.truncate(keep);
    }
    seeds.sort_by(|a, b| a.0.total_cmp(&b.0));
    let start = seeds
        .iter()
        .take(12)
        .find_map(|(_, x)| pair.solve(*x, None))
        .ok_or(if pair.r > 0.0 { "the surfaces are too far apart for this radius, or do not face each other" } else { "the surfaces do not intersect" })?;
    // March both ways along the centre line.
    let step = if pair.r > 0.0 { pair.r } else { size / 20.0 }.max(1e-3);
    let mut forward = vec![pair.ball(start)];
    let mut backward: Vec<Ball> = Vec::new();
    for dir in [1.0, -1.0] {
        let mut cur = pair.ball(start);
        let mut prev_t = scale(pair.tangent(&cur), dir);
        for _ in 0..4000 {
            let mut t = pair.tangent(&cur);
            if dot(t, prev_t) < 0.0 {
                t = scale(t, -1.0);
            }
            let mut h = step;
            let next = loop {
                let target = add(cur.c, scale(t, h));
                if let Some(x) = pair.solve(cur.x, Some((target, t))) {
                    let b = pair.ball(x);
                    if dist(b.c, cur.c) > 1e-9 && dist(b.c, cur.c) < 2.0 * h {
                        break Some(b);
                    }
                }
                h /= 2.0;
                if h < step / 64.0 {
                    break None;
                }
            };
            let Some(b) = next else {
                // Ran off a surface: put the last ball on the edge it ran into (the parameter
                // nearest its bound, held there).
                let k = (0..4)
                    .min_by(|a, b| {
                        let gap = |i: usize| cur.x[i].min(1.0 - cur.x[i]);
                        gap(*a).total_cmp(&gap(*b))
                    })
                    .unwrap();
                let mut x = cur.x;
                x[k] = if x[k] < 0.5 { 0.0 } else { 1.0 };
                if let Some(x) = self_solve_edge(pair, x, k) {
                    let b = pair.ball(x);
                    if dot(sub(b.c, cur.c), t) > 1e-9 && dist(b.c, cur.c) < 2.0 * step {
                        if dir > 0.0 {
                            forward.push(b);
                        } else {
                            backward.push(b);
                        }
                    }
                }
                break;
            };
            // Stop where a parameter reached a surface edge.
            let at_edge = b.x.iter().any(|v| *v <= 1e-9 || *v >= 1.0 - 1e-9);
            prev_t = t;
            if dir > 0.0 {
                forward.push(b);
            } else {
                backward.push(b);
            }
            cur = b;
            if at_edge {
                break;
            }
        }
    }
    backward.reverse();
    backward.extend(forward);
    let balls = backward;
    if balls.len() < 2 {
        return Err("the fillet has no length".into());
    }
    // Arc length along the centre line.
    let mut s = vec![0.0];
    for w in balls.windows(2) {
        s.push(s.last().unwrap() + dist(w[0].c, w[1].c));
    }
    let total = *s.last().unwrap();
    let s: Vec<f64> = s.iter().map(|v| v / total).collect();
    Ok((balls, s))
}

/// The ball at place `t` (exact: Newton on the plane through the interpolated centre).
fn ball_at(pair: &Pair, balls: &[Ball], s: &[f64], t: f64) -> Ball {
    let i = s.partition_point(|v| *v <= t).clamp(1, s.len() - 1) - 1;
    let f = ((t - s[i]) / (s[i + 1] - s[i]).max(1e-300)).clamp(0.0, 1.0);
    let (a, b) = (balls[i], balls[i + 1]);
    let guess: [f64; 4] = std::array::from_fn(|k| a.x[k] + (b.x[k] - a.x[k]) * f);
    let target = add(a.c, scale(sub(b.c, a.c), f));
    let tan = unit(sub(b.c, a.c));
    pair.solve(guess, Some((target, tan))).map(|x| pair.ball(x)).unwrap_or(if f < 0.5 { a } else { b })
}

/// Rhino 5 FilletSrf: the rolling-ball surface of radius `r` between `g1` and `g2`, in the
/// corner facing the points `keep1` (on g1) and `keep2` (on g2) — the sides kept.
pub fn fillet(g1: &Geo, g2: &Geo, r: f64, keep1: Vec3, keep2: Vec3, tol: f64) -> Result<Swept, String> {
    if r <= 0.0 {
        return Err("the radius must be more than 0".into());
    }
    let s1 = Srf::of(g1).ok_or("not a NURBS surface")?;
    let s2 = Srf::of(g2).ok_or("not a NURBS surface")?;
    // Sides: the ball is offset from each surface towards the other surface's kept side.
    let side_of = |s: &Srf, toward: Vec3| -> f64 {
        let (a, b) = s.closest_st(toward);
        let n = normal(s, a, b);
        if dot(sub(toward, s.at_st(a, b)), n) >= 0.0 { 1.0 } else { -1.0 }
    };
    let orient = [orientation(&s1), orientation(&s2)];
    let pair = Pair { side: [side_of(&s1, keep2), side_of(&s2, keep1)], s1, s2, r, orient };
    let size = r;
    let (balls, s) = trace(&pair, size)?;
    // The exact section anywhere: from the nearest traced ball, Newton on the plane through
    // the interpolated centre.
    let placed = |t: f64| -> Vec<H> {
        let ball = ball_at(&pair, &balls, &s, t);
        arc(ball.c, ball.p1, ball.p2)
    };
    fit_stations(&placed, 0.0, 1.0, false, s.iter().copied().step_by(4).collect(), 2, &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0], tol, 600)
}


/// Surface derivatives by central differences (one-sided at the edges).
fn derivs(s: &Srf, a: f64, b: f64) -> (Vec3, Vec3) {
    let h = 1e-6;
    let (a0, a1) = ((a - h).max(0.0), (a + h).min(1.0));
    let (b0, b1) = ((b - h).max(0.0), (b + h).min(1.0));
    (
        scale(sub(s.at_st(a1, b), s.at_st(a0, b)), 1.0 / (a1 - a0)),
        scale(sub(s.at_st(a, b1), s.at_st(a, b0)), 1.0 / (b1 - b0)),
    )
}

/// The point of `s` (parameters near `start`) `d` from `at`, in the plane through `at`
/// across `tan`, on the side of `toward`.
fn contact(s: &Srf, start: (f64, f64), at: Vec3, tan: Vec3, n: Vec3, d: f64, toward: Vec3) -> Option<Vec3> {
    let mut dir = unit(cross(n, tan));
    if dot(dir, sub(toward, at)) < 0.0 {
        dir = scale(dir, -1.0);
    }
    let target = add(at, scale(dir, d));
    let (mut a, mut b) = start;
    // Walk to the target (nearest point), then solve the two conditions exactly.
    for _ in 0..30 {
        let (su, sv) = derivs(s, a, b);
        let r = sub(target, s.at_st(a, b));
        let m = [[dot(su, su), dot(su, sv)], [dot(su, sv), dot(sv, sv)]];
        let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        if det.abs() < 1e-300 {
            break;
        }
        let (ra, rb) = (dot(su, r), dot(sv, r));
        let da = (m[1][1] * ra - m[0][1] * rb) / det;
        let db = (m[0][0] * rb - m[1][0] * ra) / det;
        a = (a + da).clamp(0.0, 1.0);
        b = (b + db).clamp(0.0, 1.0);
        if da.abs() + db.abs() < 1e-13 {
            break;
        }
    }
    for _ in 0..30 {
        let p = s.at_st(a, b);
        let q = sub(p, at);
        let g = [dot(q, tan), dot(q, q) - d * d];
        if g[0].abs() + g[1].abs() < 1e-12 * d.max(1.0) {
            return Some(p);
        }
        let (su, sv) = derivs(s, a, b);
        let j = [[dot(su, tan), dot(sv, tan)], [2.0 * dot(q, su), 2.0 * dot(q, sv)]];
        let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
        if det.abs() < 1e-300 {
            return None;
        }
        let da = (-g[0] * j[1][1] + g[1] * j[0][1]) / det;
        let db = (-g[1] * j[0][0] + g[0] * j[1][0]) / det;
        a = (a + da).clamp(0.0, 1.0);
        b = (b + db).clamp(0.0, 1.0);
    }
    let p = s.at_st(a, b);
    let q = sub(p, at);
    (dot(q, tan).abs() < 1e-7 && (norm(q) - d).abs() < 1e-7).then_some(p)
}

/// Rhino 5 ChamferSrf: a ruled surface from `d1` on `g1` to `d2` on `g2`, measured from their
/// intersection across it, on the sides kept (`keep1`, `keep2`).
pub fn chamfer(g1: &Geo, g2: &Geo, d1: f64, d2: f64, keep1: Vec3, keep2: Vec3, tol: f64) -> Result<Swept, String> {
    if d1 <= 0.0 || d2 <= 0.0 {
        return Err("the distances must be more than 0".into());
    }
    let s1 = Srf::of(g1).ok_or("not a NURBS surface")?;
    let s2 = Srf::of(g2).ok_or("not a NURBS surface")?;
    let orient = [orientation(&s1), orientation(&s2)];
    let size = dist(s1.at_st(0.0, 0.0), s1.at_st(1.0, 1.0)).max(dist(s2.at_st(0.0, 0.0), s2.at_st(1.0, 1.0))).max(d1).max(d2);
    let pair = Pair { side: [1.0, 1.0], s1, s2, r: 0.0, orient };
    let (balls, s) = trace(&pair, size)?;
    let placed = |t: f64| -> Vec<H> {
        let b = ball_at(&pair, &balls, &s, t);
        let tan = pair.tangent(&b);
        let p1 = contact(&pair.s1, (b.x[0], b.x[1]), b.c, tan, pair.n1(b.x[0], b.x[1]), d1, keep1);
        let p2 = contact(&pair.s2, (b.x[2], b.x[3]), b.c, tan, pair.n2(b.x[2], b.x[3]), d2, keep2);
        match (p1, p2) {
            (Some(p), Some(q)) => vec![[p[0], p[1], p[2], 1.0], [q[0], q[1], q[2], 1.0]],
            _ => vec![[b.c[0], b.c[1], b.c[2], 1.0]; 2],
        }
    };
    // Every traced place must have both contacts (else the distance runs off a surface).
    for t in &s {
        let row = placed(*t);
        if dist(crate::exact::euclid(row[0]), crate::exact::euclid(row[1])) < 1e-12 {
            return Err("a chamfer distance runs off a surface".into());
        }
    }
    fit_stations(&placed, 0.0, 1.0, false, s.iter().copied().step_by(4).collect(), 1, &[0.0, 0.0, 1.0, 1.0], tol, 600)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact::{self, Nc};

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
    fn fillet_between_two_planes_is_a_quarter_cylinder() {
        // Floor z = 0 (x 0..10, y 0..8) and wall x = 0 (z 0..6, y 0..8); keep the floor at
        // x > 0 and the wall at z > 0: the fillet r = 1 is centred on (1, y, 1).
        let floor = plane([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 8.0, 0.0]);
        let wall = plane([0.0, 0.0, 0.0], [0.0, 0.0, 6.0], [0.0, 8.0, 0.0]);
        let t0 = std::time::Instant::now();
        let f = fillet(&floor, &wall, 1.0, [5.0, 4.0, 0.0], [0.0, 4.0, 3.0], 0.001).unwrap();
        eprintln!("fillet planes: {:?}, {} stations, dev {:.2e}", t0.elapsed(), f.stations, f.deviation);
        for i in 0..=20 {
            for j in 0..=10 {
                let p = eval(&f, i as f64 / 20.0, j as f64 / 10.0);
                let d = ((p[0] - 1.0).powi(2) + (p[2] - 1.0).powi(2)).sqrt();
                assert!((d - 1.0).abs() < 1e-6, "{p:?}");
                assert!(p[0] <= 1.0 + 1e-9 && p[2] <= 1.0 + 1e-9, "the inside corner: {p:?}");
            }
            // Runs the whole length of the edge.
            let _ = i;
        }
        let (a, b) = (eval(&f, 0.0, 0.5), eval(&f, 1.0, 0.5));
        assert!((a[1] - b[1]).abs() > 7.99, "{a:?} {b:?}");
    }

    #[test]
    fn fillet_between_a_cylinder_and_a_plane() {
        // Cylinder r3 along z standing on the floor: fillet r 0.5 round its foot (outside).
        let ring = exact::arc([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 3.0, std::f64::consts::TAU);
        let rows: Vec<Vec<H>> = ring.points.iter().map(|c| vec![*c, [c[0], c[1], 5.0 * c[3], c[3]]]).collect();
        let cyl = exact::surface(&rows, [2, 1], [ring.knots.clone(), vec![0.0, 0.0, 1.0, 1.0]]);
        let floor = plane([-8.0, -8.0, 0.0], [16.0, 0.0, 0.0], [0.0, 16.0, 0.0]);
        let t0 = std::time::Instant::now();
        let f = fillet(&cyl, &floor, 0.5, [3.0, 0.0, 2.0], [6.0, 0.0, 0.0], 0.001).unwrap();
        eprintln!("fillet cylinder: {:?}, {} stations, dev {:.2e}", t0.elapsed(), f.stations, f.deviation);
        assert!(f.deviation <= 0.001);
        // Every point is 0.5 from the ball centre circle (radius 3.5 at height 0.5).
        for i in 0..=60 {
            for j in 0..=8 {
                let p = eval(&f, i as f64 / 60.0, j as f64 / 8.0);
                let d = (p[0].hypot(p[1]) - 3.5).hypot(p[2] - 0.5);
                assert!((d - 0.5).abs() < 0.002, "{d}");
            }
        }
    }

    #[test]
    fn chamfer_between_two_planes() {
        let floor = plane([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 8.0, 0.0]);
        let wall = plane([0.0, 0.0, 0.0], [0.0, 0.0, 6.0], [0.0, 8.0, 0.0]);
        let c = chamfer(&floor, &wall, 1.0, 2.0, [5.0, 4.0, 0.0], [0.0, 4.0, 3.0], 0.001).unwrap();
        for i in 0..=10 {
            let (a, b) = (eval(&c, i as f64 / 10.0, 0.0), eval(&c, i as f64 / 10.0, 1.0));
            assert!(a[2].abs() < 1e-9 && (a[0] - 1.0).abs() < 1e-9, "on the floor 1 from the corner: {a:?}");
            assert!(b[0].abs() < 1e-9 && (b[2] - 2.0).abs() < 1e-9, "on the wall 2 up: {b:?}");
        }
    }
}
