// SPDX-License-Identifier: LGPL-2.1-or-later
//! Rhino 5 Sweep1 on exact curves: cross sections are moved along one rail by frames and
//! the surface passes exactly through every placed section.
//!
//! - Frames: **Freeform** uses a rotation-minimising frame (double reflection, Wang et al.
//!   2008), so a section keeps its angle to the rail; **Roadlike** builds x from the rail
//!   tangent and a fixed direction (Rhino 5 help, "Technical notes": x = tangent × fixed,
//!   y ⟂ both, z = tangent).
//! - Shape between sections: sections are expressed in their frames, made compatible (same
//!   degree and knots, shapes unchanged) and blended pole by pole — smoothly (constant near
//!   each section) or linearly (Rhino's *Global shape blending*).
//! - Stations along the rail are added until the surface is within `tol` of the true sweep
//!   everywhere between them; the surface interpolates every station exactly.
//! - A straight rail gives an exact extrusion (degree 1 along the rail); a circular rail with
//!   one shape gives an exact rational surface of revolution.
#![allow(clippy::needless_range_loop, clippy::collapsible_if)]
use crate::exact::{self, H, Nc, euclid};
use om9_addon_abi::session::{Vec3, add, cross, dist, dot, norm, scale, sub, unit};
use std::f64::consts::TAU;

/// How the frames along the rail are made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    Freeform,
    /// Roadlike with the fixed direction (Top: world z, Right: x, Front: y).
    Roadlike(Vec3),
}

#[derive(Debug, Clone)]
pub struct Options {
    pub style: Style,
    /// Closed sweep (closed rail, two or more sections): continue past the last section
    /// around to the first. One section on a closed rail always goes all the way round.
    pub closed: bool,
    pub global_blend: bool,
    /// Largest distance from the true sweep (mm).
    pub tol: f64,
    /// Most stations along the rail.
    pub max_stations: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            style: Style::Freeform,
            closed: false,
            global_blend: false,
            tol: 0.001,
            max_stations: 400,
        }
    }
}

/// A swept surface: pole rows along the rail (u) by along the sections (v).
#[derive(Debug, Clone)]
pub struct Swept {
    pub rows: Vec<Vec<H>>,
    pub degree: [usize; 2],
    pub knots: [Vec<f64>; 2],
    /// Largest measured distance from the true sweep (0 for the exact cases).
    pub deviation: f64,
    pub stations: usize,
    /// Extrusion or revolution: exact by construction.
    pub exact: bool,
}

impl Swept {
    pub fn geo(&self) -> om9_addon_abi::session::Geo {
        exact::surface(&self.rows, self.degree, self.knots.clone())
    }
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    o: Vec3,
    x: Vec3,
    y: Vec3,
    z: Vec3,
}

impl Frame {
    fn local(&self, p: H) -> H {
        let w = p[3];
        let d = sub(euclid(p), self.o);
        [dot(d, self.x) * w, dot(d, self.y) * w, dot(d, self.z) * w, w]
    }
    fn world(&self, l: H) -> H {
        let w = l[3];
        let q = euclid(l);
        let p = add(self.o, add(scale(self.x, q[0]), add(scale(self.y, q[1]), scale(self.z, q[2]))));
        [p[0] * w, p[1] * w, p[2] * w, w]
    }
}

fn tangent(c: &Nc, u: f64) -> Vec3 {
    for h in [1e-7, 1e-5, 1e-3] {
        let (a, b) = ((u - h).max(0.0), (u + h).min(1.0));
        let d = sub(c.eval(b), c.eval(a));
        if norm(d) > 1e-12 {
            return unit(d);
        }
    }
    [0.0, 0.0, 1.0]
}

fn perpendicular(t: Vec3) -> Vec3 {
    let a = if t[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    unit(sub(a, scale(t, dot(a, t))))
}

/// One double-reflection step of the reference vector `r` from (x0, t0) to (x1, t1).
fn reflect(x0: Vec3, t0: Vec3, r: Vec3, x1: Vec3, t1: Vec3) -> Vec3 {
    let v1 = sub(x1, x0);
    let c1 = dot(v1, v1);
    let (rl, tl) = if c1 > 1e-24 {
        (sub(r, scale(v1, 2.0 / c1 * dot(v1, r))), sub(t0, scale(v1, 2.0 / c1 * dot(v1, t0))))
    } else {
        (r, t0)
    };
    let v2 = sub(t1, tl);
    let c2 = dot(v2, v2);
    let r1 = if c2 > 1e-24 { sub(rl, scale(v2, 2.0 / c2 * dot(v2, rl))) } else { rl };
    // Keep it exactly perpendicular to the tangent.
    let r1 = sub(r1, scale(t1, dot(r1, t1)));
    if norm(r1) < 1e-12 { perpendicular(t1) } else { unit(r1) }
}

/// Frames along the rail.
struct Rail {
    c: Nc,
    style: Style,
    /// Freeform: (u, point, tangent, reference) on a fine grid.
    grid: Vec<(f64, Vec3, Vec3, Vec3)>,
    /// Freeform on a closed sweep: turn removed per unit of u (the frame comes back to itself).
    twist: f64,
}

impl Rail {
    fn new(c: Nc, style: Style, wrap: bool) -> Rail {
        let g = 2048;
        let mut grid = Vec::with_capacity(g + 1);
        let mut twist = 0.0;
        if style == Style::Freeform {
            let t0 = tangent(&c, 0.0);
            let mut prev = (0.0, c.eval(0.0), t0, perpendicular(t0));
            grid.push(prev);
            for i in 1..=g {
                let u = i as f64 / g as f64;
                let (x1, t1) = (c.eval(u), tangent(&c, u));
                let r1 = reflect(prev.1, prev.2, prev.3, x1, t1);
                prev = (u, x1, t1, r1);
                grid.push(prev);
            }
            if wrap {
                // Angle from the start reference to where it came back, about the tangent.
                let (r0, t0) = (grid[0].3, grid[0].2);
                let r1 = grid[g].3;
                twist = dot(cross(r0, r1), t0).atan2(dot(r0, r1));
            }
        }
        Rail { c, style, grid, twist }
    }

    fn frame(&self, u: f64) -> Frame {
        let o = self.c.eval(u);
        let z = tangent(&self.c, u);
        let x = match self.style {
            Style::Freeform => {
                let g = self.grid.len() - 1;
                let i = ((u * g as f64).floor() as usize).min(g);
                let (_, x0, t0, r0) = self.grid[i];
                let r = reflect(x0, t0, r0, o, z);
                if self.twist != 0.0 {
                    // Rotate about z by −twist·u (Rodrigues; r ⟂ z).
                    let a = -self.twist * u;
                    add(scale(r, a.cos()), scale(cross(z, r), a.sin()))
                } else {
                    r
                }
            }
            Style::Roadlike(d) => {
                let x = cross(z, unit(d));
                if norm(x) > 1e-9 {
                    unit(x)
                } else {
                    // Tangent along the fixed direction: the frame is under-defined (Rhino lets
                    // it twist); keep any perpendicular.
                    perpendicular(z)
                }
            }
        };
        Frame { o, x, y: cross(z, x), z }
    }
}

/// Where a section sits on the rail: where it touches the rail, else its middle's nearest.
fn section_param(rail: &Nc, s: &Nc) -> f64 {
    let pts: Vec<Vec3> = (0..=48).map(|i| s.eval(i as f64 / 48.0)).collect();
    let size = pts.iter().map(|p| dist(*p, pts[0])).fold(0.0, f64::max).max(1e-9);
    let mut best = (f64::MAX, 0.0);
    for p in &pts {
        let u = rail.closest(*p);
        let d = dist(rail.eval(u), *p);
        if d < best.0 {
            best = (d, u);
        }
    }
    if best.0 <= (1e-3 * size).max(1e-4) {
        return best.1;
    }
    let mid = scale(pts.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / pts.len() as f64);
    rail.closest(mid)
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Is the rail (over its whole domain) part of a circle? Center, axis (right-handed with
/// the rail's direction) and radius.
fn circle_of(c: &Nc) -> Option<(Vec3, Vec3, f64)> {
    let (a, b, d) = (c.eval(0.0), c.eval(1.0 / 3.0), c.eval(2.0 / 3.0));
    let (ab, ad) = (sub(b, a), sub(d, a));
    let n = cross(ab, ad);
    let nn = dot(n, n);
    if nn < 1e-18 {
        return None;
    }
    // Circumcenter of a, b, d.
    let center = add(
        a,
        scale(add(scale(cross(n, ab), dot(ad, ad)), scale(cross(ad, n), dot(ab, ab))), 0.5 / nn),
    );
    let r = dist(center, a);
    let axis = unit(n);
    let ok = (0..=128).all(|i| {
        let p = c.eval(i as f64 / 128.0);
        (dist(p, center) - r).abs() <= 1e-9 * r.max(1.0) && dot(sub(p, center), axis).abs() <= 1e-9 * r.max(1.0)
    });
    ok.then_some((center, axis, r))
}

fn is_line(c: &Nc) -> bool {
    let (a, b) = (c.start(), c.end());
    let l = dist(a, b);
    if l < 1e-9 {
        return false;
    }
    let d = scale(sub(b, a), 1.0 / l);
    (0..=64).all(|i| {
        let p = sub(c.eval(i as f64 / 64.0), a);
        norm(sub(p, scale(d, dot(p, d)))) <= 1e-9 * l.max(1.0)
    })
}

/// Rows of an exact revolution of `section` (world poles) about `axis` through `center` by
/// `angle` (rational quadratic arcs, at most 90° each).
fn revolve_rows(section: &[H], center: Vec3, axis: Vec3, angle: f64) -> (Vec<Vec<H>>, Vec<f64>) {
    let arcs = (angle.abs() / std::f64::consts::FRAC_PI_2 - 1e-9).ceil().max(1.0) as usize;
    let d = angle / arcs as f64;
    let wm = (d / 2.0).cos();
    let mut knots = vec![0.0; 3];
    for k in 1..arcs {
        let t = k as f64 / arcs as f64;
        knots.extend([t, t]);
    }
    knots.extend([1.0; 3]);
    let rot = |p: Vec3, th: f64, stretch: f64| -> Vec3 {
        let c = add(center, scale(axis, dot(sub(p, center), axis)));
        let r = sub(p, c);
        let y = cross(axis, r);
        add(c, scale(add(scale(r, th.cos()), scale(y, th.sin())), stretch))
    };
    let rows = (0..=2 * arcs)
        .map(|i| {
            let th = d * i as f64 / 2.0;
            let mid = i % 2 == 1;
            section
                .iter()
                .map(|hp| {
                    let p = rot(euclid(*hp), th, if mid { 1.0 / wm } else { 1.0 });
                    let w = hp[3] * if mid { wm } else { 1.0 };
                    [p[0] * w, p[1] * w, p[2] * w, w]
                })
                .collect()
        })
        .collect();
    (rows, knots)
}

/// The iso curve of rows (degree `q`, `knots` across) at `t`: homogeneous poles.
fn iso(rows: &[Vec<H>], q: usize, knots: &[f64], t: f64) -> Vec<H> {
    let width = rows[0].len();
    (0..width)
        .map(|j| {
            let c = Nc {
                degree: q,
                points: rows.iter().map(|r| r[j]).collect(),
                knots: knots.to_vec(),
            };
            c.eval_h(t)
        })
        .collect()
}

/// Rhino 5 Sweep1. `sections` in the order picked (curves; a point section is a constant
/// curve, allowed first or last). Returns the surface or why not.
pub fn sweep1(rail: &Nc, sections: &[Nc], o: &Options) -> Result<Swept, String> {
    if sections.is_empty() {
        return Err("select the cross section curves".into());
    }
    let rail_closed = rail.closed(1e-6);
    let mut rail = rail.clone();
    // Closed rail: start at the first section and run towards the second (Rhino: the first
    // section picked is where a closed sweep starts and ends).
    let mut params: Vec<f64>;
    if rail_closed {
        let s0 = section_param(&rail, &sections[0]);
        rail = rail.with_seam(s0);
        params = sections.iter().map(|s| section_param(&rail, s)).collect();
        params[0] = 0.0;
        if sections.len() >= 2 {
            let second = params[1];
            if second > 0.5 {
                // Picked against the rail's direction (most of the way round is shorter the
                // other way): run the rail backwards.
                let ordered_back = params.windows(2).skip(1).all(|w| w[1] <= w[0] + 1e-9);
                if ordered_back {
                    rail = rail.reversed();
                    params = sections.iter().map(|s| section_param(&rail, s)).collect();
                    params[0] = 0.0;
                }
            }
        }
        for p in params.iter_mut().skip(1) {
            if *p < 1e-9 {
                *p = 1.0;
            }
        }
    } else {
        params = sections.iter().map(|s| section_param(&rail, s)).collect();
    }
    let wrap = rail_closed && (sections.len() == 1 || o.closed);
    if o.closed && !rail_closed {
        return Err("a closed sweep needs a closed rail".into());
    }
    // Sort the sections along the rail.
    let mut order: Vec<usize> = (0..sections.len()).collect();
    order.sort_by(|&a, &b| params[a].total_cmp(&params[b]));
    let is_point = |c: &Nc| c.points.iter().all(|p| dist(euclid(*p), euclid(c.points[0])) < 1e-12);
    for (k, &i) in order.iter().enumerate() {
        if is_point(&sections[i]) && k != 0 && k + 1 != order.len() {
            return Err("a point can only start or end the sweep".into());
        }
    }
    if wrap && sections.iter().any(is_point) {
        return Err("a closed sweep cannot have a point section".into());
    }
    let at: Vec<f64> = order.iter().map(|&i| params[i]).collect();
    // The range of the rail swept.
    let (ua, ub) = if wrap {
        (0.0, 1.0)
    } else if rail_closed {
        (0.0, *at.last().unwrap())
    } else {
        (0.0, 1.0)
    };
    if ub - ua < 1e-9 {
        return Err("the sections are all at the same place on the rail".into());
    }
    let frames = Rail::new(rail.clone(), o.style, wrap);
    // Sections in their frames, aligned and compatible.
    let mut locals: Vec<Nc> = order
        .iter()
        .zip(&at)
        .map(|(&i, &s)| {
            let f = frames.frame(s);
            Nc {
                degree: sections[i].degree,
                points: sections[i].points.iter().map(|p| f.local(*p)).collect(),
                knots: sections[i].knots.clone(),
            }
        })
        .collect();
    let curve_closed: Vec<bool> = locals.iter().map(|c| !is_point(c) && c.closed(1e-6)).collect();
    let all_closed = curve_closed.iter().zip(&locals).all(|(c, l)| *c || is_point(l));
    let any_closed = curve_closed.iter().any(|c| *c);
    if any_closed && !all_closed {
        return Err("the cross sections must be all closed or all open".into());
    }
    exact::align_sections(&mut locals, all_closed && any_closed);
    exact::make_compatible(&mut locals);
    // A point section takes its neighbour's weights (still the same point), so the shape
    // shrinks to it without bending: a circle to a point is a cone.
    for k in 0..locals.len() {
        if is_point(&locals[k]) && locals.len() > 1 {
            let other = if k == 0 { 1 } else { k - 1 };
            let ws: Vec<f64> = locals[other].points.iter().map(|q| q[3]).collect();
            let at = euclid(locals[k].points[0]);
            for (pt, w) in locals[k].points.iter_mut().zip(ws) {
                *pt = [at[0] * w, at[1] * w, at[2] * w, w];
            }
        }
    }
    let (p, sec_knots) = (locals[0].degree, locals[0].knots.clone());
    let m = locals.len();

    // The local shape at rail parameter u.
    let shape = |u: f64| -> Vec<H> {
        if m == 1 {
            return locals[0].points.clone();
        }
        let blend = |a: usize, b: usize, t: f64| -> Vec<H> {
            let t = if o.global_blend { t } else { smooth(t.clamp(0.0, 1.0)) };
            // Points and weights blended apart (a homogeneous blend would bend the shape
            // when the weights differ).
            locals[a]
                .points
                .iter()
                .zip(&locals[b].points)
                .map(|(x, y)| {
                    let (p, q) = (euclid(*x), euclid(*y));
                    let w = x[3] + (y[3] - x[3]) * t;
                    let e = add(p, scale(sub(q, p), t));
                    [e[0] * w, e[1] * w, e[2] * w, w]
                })
                .collect()
        };
        for k in 0..m - 1 {
            if u >= at[k] && u <= at[k + 1] {
                let span = (at[k + 1] - at[k]).max(1e-12);
                return blend(k, k + 1, (u - at[k]) / span);
            }
        }
        if wrap {
            // Between the last section and the first one again (at u + 1).
            let (last, first) = (at[m - 1], at[0] + 1.0);
            let uu = if u < at[0] { u + 1.0 } else { u };
            return blend(m - 1, 0, (uu - last) / (first - last).max(1e-12));
        }
        if u < at[0] { locals[0].points.clone() } else { locals[m - 1].points.clone() }
    };
    let placed = |u: f64| -> Vec<H> {
        let f = frames.frame(u);
        shape(u).iter().map(|l| f.world(*l)).collect()
    };
    let constant_shape = m == 1
        || (1..m).all(|k| {
            locals[k]
                .points
                .iter()
                .zip(&locals[0].points)
                .all(|(a, b)| dist(euclid(*a), euclid(*b)) < 1e-9 && (a[3] - b[3]).abs() < 1e-12)
        });

    // Exact cases.
    if constant_shape && is_line(&rail) {
        let rows = vec![placed(ua), placed(ub)];
        return Ok(Swept {
            rows,
            degree: [1, p],
            knots: [vec![0.0, 0.0, 1.0, 1.0], sec_knots],
            deviation: 0.0,
            stations: 2,
            exact: true,
        });
    }
    if constant_shape {
        if let Some((center, axis, _)) = circle_of(&rail) {
            let fits_style = match o.style {
                Style::Freeform => true,
                Style::Roadlike(d) => norm(cross(unit(d), axis)) < 1e-9,
            };
            if fits_style {
                // Angle swept from rail(ua) to rail(ub) about the axis, along the rail.
                let angle = if wrap {
                    TAU
                } else {
                    let (a, b) = (sub(rail.eval(ua), center), sub(rail.eval(ub), center));
                    let mut t = dot(cross(a, b), axis).atan2(dot(a, b));
                    if t <= 1e-12 {
                        t += TAU;
                    }
                    t
                };
                let (rows, ku) = revolve_rows(&placed(ua), center, axis, angle);
                return Ok(Swept {
                    stations: rows.len(),
                    rows,
                    degree: [2, p],
                    knots: [ku, sec_knots],
                    deviation: 0.0,
                    exact: true,
                });
            }
        }
    }

    // General case: stations refined until within tolerance.
    let mut stations: Vec<f64> = vec![ua, ub];
    stations.extend(at.iter().copied().filter(|u| *u > ua && *u < ub));
    for k in &rail.knots {
        if *k > ua && *k < ub {
            stations.push(*k);
        }
    }
    let base = if wrap { 12 } else { 8 };
    stations.extend((1..base).map(|i| ua + (ub - ua) * i as f64 / base as f64));
    let tidy = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-7 * (ub - ua));
    };
    tidy(&mut stations);
    let width_samples = (4 * locals[0].points.len()).clamp(16, 96);
    let mut best: Option<Swept> = None;
    loop {
        let used: Vec<f64> = if wrap { stations[..stations.len() - 1].to_vec() } else { stations.clone() };
        let secs: Vec<Nc> = used
            .iter()
            .map(|u| Nc { degree: p, points: placed(*u), knots: sec_knots.clone() })
            .collect();
        let ts: Vec<f64> = stations.iter().map(|u| (u - ua) / (ub - ua)).collect();
        let Some((rows, ku)) = exact::interpolate_across(&secs, &ts, 3, wrap) else {
            return best.ok_or_else(|| "could not fit the sweep".to_string());
        };
        let q = 3.min(secs.len() - 1).max(1);
        // Check the middle of every interval.
        let mut worst = 0.0f64;
        let mut split = Vec::new();
        for w in stations.windows(2) {
            let u = (w[0] + w[1]) / 2.0;
            let t = (u - ua) / (ub - ua);
            let got = Nc { degree: p, points: iso(&rows, q, &ku, t), knots: sec_knots.clone() };
            let want = Nc { degree: p, points: placed(u), knots: sec_knots.clone() };
            let d = (0..=width_samples)
                .map(|i| {
                    let v = i as f64 / width_samples as f64;
                    dist(got.eval(v), want.eval(v))
                })
                .fold(0.0, f64::max);
            worst = worst.max(d);
            if d > o.tol {
                split.push(u);
            }
        }
        let done = Swept {
            stations: stations.len(),
            rows,
            degree: [q, p],
            knots: [ku, sec_knots.clone()],
            deviation: worst,
            exact: false,
        };
        if split.is_empty() || stations.len() + split.len() > o.max_stations {
            return Ok(done);
        }
        best = Some(done);
        stations.extend(split);
        tidy(&mut stations);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use om9_addon_abi::session::Geo;

    fn circle(c: Vec3, x: Vec3, y: Vec3, r: f64) -> Nc {
        exact::arc(c, x, y, r, TAU)
    }

    fn srf_eval(s: &Swept, u: f64, v: f64) -> Vec3 {
        let row = iso(&s.rows, s.degree[0], &s.knots[0], u);
        Nc { degree: s.degree[1], points: row, knots: s.knots[1].clone() }.eval(v)
    }

    #[test]
    fn a_circle_round_a_circle_is_an_exact_torus() {
        // Rail: R10 in XY; section: r1 in the XZ plane at (10, 0, 0).
        let rail = circle([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 10.0);
        let sec = circle([10.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0);
        let s = sweep1(&rail, &[sec], &Options::default()).unwrap();
        assert!(s.exact);
        for i in 0..=60 {
            for j in 0..=40 {
                let p = srf_eval(&s, i as f64 / 60.0, j as f64 / 40.0);
                let ring = ((p[0] * p[0] + p[1] * p[1]).sqrt() - 10.0).hypot(p[2]);
                assert!((ring - 1.0).abs() < 1e-9, "{ring}");
            }
        }
    }

    #[test]
    fn a_section_along_a_line_is_an_exact_extrusion() {
        let rail = Nc::from_geo(&Geo::Line { start: [0.0; 3], end: [0.0, 0.0, 7.0] }).unwrap();
        let sec = circle([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 2.0);
        let s = sweep1(&rail, &[sec], &Options::default()).unwrap();
        assert!(s.exact && s.degree[0] == 1);
        let p = srf_eval(&s, 0.5, 0.3);
        assert!(((p[0].hypot(p[1])) - 2.0).abs() < 1e-12 && (p[2] - 3.5).abs() < 1e-12);
    }

    #[test]
    fn a_free_curve_rail_keeps_the_section_shape_within_tolerance() {
        // A wavy 3-D rail, one r1 circle: every cross-section of the sweep must stay a unit
        // circle around the rail (Freeform keeps the angle to the rail).
        let pts: Vec<Vec3> = (0..9).map(|i| {
            let t = i as f64;
            [t * 3.0, 4.0 * (t * 0.7).sin(), 2.0 * (t * 0.4).cos()]
        }).collect();
        let rail = Nc::from_geo(&crate::interpolate(&pts, 3, false).unwrap()).unwrap();
        let o = rail.eval(0.0);
        let t = tangent(&rail, 0.0);
        let x = perpendicular(t);
        let sec = circle(o, x, cross(t, x), 1.0);
        let opts = Options::default();
        let s = sweep1(&rail, &[sec], &opts).unwrap();
        assert!(s.deviation <= opts.tol, "{}", s.deviation);
        // Every surface point is at distance 1 from the rail (up to the tolerance).
        for i in 0..=80 {
            for j in 0..=16 {
                let p = srf_eval(&s, i as f64 / 80.0, j as f64 / 16.0);
                let d = dist(p, rail.eval(rail.closest(p)));
                assert!((d - 1.0).abs() < 3.0 * opts.tol, "{d} at {i},{j}");
            }
        }
    }

    #[test]
    fn two_sections_are_on_the_surface_and_closed_sweep_wraps() {
        // Ring rail with a small and a large circle opposite each other.
        let rail = circle([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 10.0);
        let a = circle([10.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0);
        let b = circle([-10.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 2.0);
        for closed in [false, true] {
            let o = Options { closed, ..Default::default() };
            let s = sweep1(&rail, &[a.clone(), b.clone()], &o).unwrap();
            assert!(s.deviation <= o.tol, "{}", s.deviation);
            // Both sections lie on the surface.
            for (sec, r) in [(&a, 1.0), (&b, 2.0)] {
                for j in 0..=20 {
                    let p = sec.eval(j as f64 / 20.0);
                    let mut best = f64::MAX;
                    // Coarse scan, then refine around the best (u, v).
                    let (mut bu, mut bv) = (0.0, 0.0);
                    for i in 0..=120 {
                        for k in 0..=24 {
                            let (u, v) = (i as f64 / 120.0, k as f64 / 24.0);
                            let d = dist(p, srf_eval(&s, u, v));
                            if d < best {
                                (best, bu, bv) = (d, u, v);
                            }
                        }
                    }
                    for i in -20..=20 {
                        for k in -20..=20 {
                            let (u, v) = ((bu + i as f64 / 2400.0).clamp(0.0, 1.0), (bv + k as f64 / 480.0).clamp(0.0, 1.0));
                            best = best.min(dist(p, srf_eval(&s, u, v)));
                        }
                    }
                    assert!(best < 0.005, "{best}");
                }
            }
            let first = srf_eval(&s, 0.0, 0.25);
            let last = srf_eval(&s, 1.0, 0.25);
            if closed {
                assert!(dist(first, last) < 1e-9, "closed sweep must come back");
            } else {
                // Open on a closed rail: from the first section to the second (half way).
                assert!((last[0] + 10.0).abs() < 2.5 && (first[0] - 10.0).abs() < 1.5, "{first:?} {last:?}");
            }
        }
    }

    #[test]
    fn sweep_speed() {
        // Release timing (printed): ring with two sections, wavy rail with one.
        let rail = circle([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 10.0);
        let a = circle([10.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0);
        let b = circle([-10.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 2.0);
        let t = std::time::Instant::now();
        let s = sweep1(&rail, &[a, b], &Options { closed: true, ..Default::default() }).unwrap();
        eprintln!("ring 2 sections: {:?}, {} stations, dev {:.2e}", t.elapsed(), s.stations, s.deviation);
    }

    #[test]
    fn roadlike_top_keeps_sections_upright() {
        // A rising helix-like rail: Roadlike Top keeps frame x horizontal.
        let pts: Vec<Vec3> = (0..12).map(|i| {
            let a = i as f64 * 0.5;
            [8.0 * a.cos(), 8.0 * a.sin(), i as f64]
        }).collect();
        let rail = Nc::from_geo(&crate::interpolate(&pts, 3, false).unwrap()).unwrap();
        let f = Rail::new(rail.clone(), Style::Roadlike([0.0, 0.0, 1.0]), false);
        for i in 0..=20 {
            let fr = f.frame(i as f64 / 20.0);
            assert!(fr.x[2].abs() < 1e-9, "x must stay horizontal");
        }
    }
}
