// SPDX-License-Identifier: LGPL-2.1-or-later
//! Polycurves sent to add-ons as one exact curve (`session::addon_polycurve`).
use om9_addon_abi::session::{self as s, Geo, Vec3};
use om9_app::session::addon_polycurve::{joined, reversed};

type H = [f64; 4];
fn h(p: Vec3, w: f64) -> H {
    [p[0] * w, p[1] * w, p[2] * w, w]
}
fn euclid(p: H) -> Vec3 {
    [p[0] / p[3], p[1] / p[3], p[2] / p[3]]
}


/// de Boor evaluation of a rational NURBS (test only).
fn eval(g: &Geo, u: f64) -> Vec3 {
    let Geo::Nurbs { degree: p, points, weights, knots } = g else { panic!("{g:?}") };
    let p = *p;
    let n = points.len();
    let u = u.clamp(knots[p], knots[n]);
    let mut k = p;
    while k < n - 1 && knots[k + 1] <= u {
        k += 1;
    }
    let w = |i: usize| weights.get(i).copied().unwrap_or(1.0);
    let mut d: Vec<H> = (0..=p).map(|j| h(points[k - p + j], w(k - p + j))).collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = k - p + j;
            let den = knots[i + p + 1 - r] - knots[i];
            let a = if den.abs() < 1e-300 { 0.0 } else { (u - knots[i]) / den };
            d[j] = std::array::from_fn(|c| (1.0 - a) * d[j - 1][c] + a * d[j][c]);
        }
    }
    euclid(d[p])
}

#[test]
fn a_rounded_rectangle_is_exact_with_few_points() {
    // 10 × 6 with r = 1 corners: 4 lines and 4 quarter arcs.
    let r = 1.0;
    let (w, hgt) = (10.0, 6.0);
    let line = |a: Vec3, b: Vec3| Geo::Line { start: a, end: b };
    let arc = |c: Vec3, a0: f64| Geo::Arc {
        center: c,
        x: [a0.cos(), a0.sin(), 0.0],
        y: [-a0.sin(), a0.cos(), 0.0],
        radius: r,
        angle: std::f64::consts::FRAC_PI_2,
    };
    use std::f64::consts::{FRAC_PI_2, PI};
    let parts = vec![
        line([r, 0.0, 0.0], [w - r, 0.0, 0.0]),
        arc([w - r, r, 0.0], -FRAC_PI_2),
        line([w, r, 0.0], [w, hgt - r, 0.0]),
        arc([w - r, hgt - r, 0.0], 0.0),
        line([w - r, hgt, 0.0], [r, hgt, 0.0]),
        arc([r, hgt - r, 0.0], FRAC_PI_2),
        line([0.0, hgt - r, 0.0], [0.0, r, 0.0]),
        arc([r, r, 0.0], PI),
    ];
    let g = joined(&parts).unwrap();
    let Geo::Nurbs { degree, points, .. } = &g else { panic!() };
    assert_eq!(*degree, 2);
    assert_eq!(points.len(), 17, "2 per piece + 1");
    // Every point lies on the rounded rectangle.
    for i in 0..=800 {
        let p = eval(&g, i as f64 / 800.0);
        let qx = (p[0] - w / 2.0).abs() - (w / 2.0 - r);
        let qy = (p[1] - hgt / 2.0).abs() - (hgt / 2.0 - r);
        let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
        assert!(d.abs() < 1e-9, "off the outline by {d} at {p:?}");
    }
    assert!(s::dist(eval(&g, 0.0), eval(&g, 1.0)) < 1e-12, "closed");
}

#[test]
fn lines_only_make_a_polyline_and_a_lower_degree_nurbs_is_refused() {
    let g = joined(&[
        Geo::Line { start: [0.0; 3], end: [1.0, 0.0, 0.0] },
        Geo::Line { start: [1.0, 0.0, 0.0], end: [1.0, 1.0, 0.0] },
    ])
    .unwrap();
    assert_eq!(g, Geo::Polyline { points: vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]], closed: false });
    // A line and a cubic: the line is raised to degree 3, the cubic kept.
    let cubic = Geo::Nurbs {
        degree: 3,
        points: vec![[1.0, 0.0, 0.0], [2.0, 1.0, 0.0], [3.0, -1.0, 0.0], [4.0, 0.0, 0.0], [5.0, 1.0, 0.0]],
        weights: vec![],
        knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
    };
    let g = joined(&[Geo::Line { start: [0.0; 3], end: [1.0, 0.0, 0.0] }, cubic.clone()]).unwrap();
    let Geo::Nurbs { points, .. } = &g else { panic!() };
    assert_eq!(points.len(), 4 + 5 - 1);
    assert!(s::dist(eval(&g, 0.25), [0.5, 0.0, 0.0]) < 1e-12);
    assert!(s::dist(eval(&g, 1.0), [5.0, 1.0, 0.0]) < 1e-12);
    // A quadratic NURBS with a cubic: cannot be raised here, so no exact curve.
    let quad = Geo::Nurbs { degree: 2, points: vec![[5.0, 1.0, 0.0], [6.0, 2.0, 0.0], [7.0, 1.0, 0.0]], weights: vec![], knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0] };
    assert!(joined(&[cubic, quad]).is_none());
}

#[test]
fn reversed_curves_run_the_other_way() {
    let arc = Geo::Arc { center: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], radius: 2.0, angle: 1.0 };
    let Geo::Arc { center, x, y, radius, angle } = reversed(arc) else { panic!() };
    let at = |t: f64| s::add(center, s::add(s::scale(x, radius * t.cos()), s::scale(y, radius * t.sin())));
    assert!(s::dist(at(0.0), [2.0 * 1f64.cos(), 2.0 * 1f64.sin(), 0.0]) < 1e-12, "starts at the old end");
    assert!(s::dist(at(angle), [2.0, 0.0, 0.0]) < 1e-12, "ends at the old start");
    let cubic = Geo::Nurbs { degree: 3, points: vec![[0.0; 3], [1.0, 1.0, 0.0], [2.0, -1.0, 0.0], [3.0, 0.0, 0.0]], weights: vec![], knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] };
    let r = reversed(cubic.clone());
    for i in 0..=10 {
        let u = i as f64 / 10.0;
        assert!(s::dist(eval(&r, u), eval(&cubic, 1.0 - u)) < 1e-12);
    }
}
