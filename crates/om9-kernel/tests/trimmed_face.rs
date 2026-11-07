// SPDX-License-Identifier: LGPL-2.1-or-later
//! Trimmed faces: a NURBS surface bounded by NURBS loops (outer boundary and holes), as
//! Rhino's PlanarSrf leaves a plane with holes.
use om9_core::{
    budget::Limits,
    geometry::{NurbsCurve, NurbsSurface},
};
use om9_kernel::creation::{self, DEFAULT_DEFLECTION_MM};

fn limits() -> Limits {
    Limits::default()
}

/// An exact circle (rational quadratic, 9 poles) in the XY plane.
fn circle(c: [f64; 3], r: f64, reversed: bool) -> NurbsCurve {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let offsets = [(1., 0.), (1., 1.), (0., 1.), (-1., 1.), (-1., 0.), (-1., -1.), (0., -1.), (1., -1.), (1., 0.)];
    NurbsCurve {
        degree: 2,
        poles: offsets.iter().map(|(x, y)| [c[0] + r * x, c[1] + r * y, c[2]]).collect(),
        weights: (0..9).map(|i| if i % 2 == 1 { w } else { 1.0 }).collect(),
        knots: vec![0., 0., 0., 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.],
        domain: [0., 1.],
        periodic: false,
        reversed,
        seam: None,
    }
}

fn line(a: [f64; 3], b: [f64; 3]) -> NurbsCurve {
    NurbsCurve {
        degree: 1,
        poles: vec![a, b],
        weights: vec![1.0; 2],
        knots: vec![0., 0., 1., 1.],
        domain: [0., 1.],
        periodic: false,
        reversed: false,
        seam: None,
    }
}

/// A flat square sheet (bilinear) from (−1, −1) to (11, 11).
fn sheet() -> NurbsSurface {
    NurbsSurface {
        degree: [1, 1],
        size: [2, 2],
        poles: vec![[-1., -1., 0.], [-1., 11., 0.], [11., -1., 0.], [11., 11., 0.]],
        weights: vec![1.0; 4],
        knots: [vec![0., 0., 1., 1.], vec![0., 0., 1., 1.]],
        domain: [[0., 1.], [0., 1.]],
        periodic: [false, false],
        reversed: false,
    }
}

#[test]
fn a_plane_with_a_round_hole_has_the_exact_area() {
    let outer = vec![
        line([0., 0., 0.], [10., 0., 0.]),
        line([10., 0., 0.], [10., 10., 0.]),
        line([10., 10., 0.], [0., 10., 0.]),
        line([0., 10., 0.], [0., 0., 0.]),
    ];
    let hole = vec![circle([5., 5., 0.], 2., false)];
    let brep = creation::trimmed_face(&sheet(), &[outer, hole], DEFAULT_DEFLECTION_MM, &limits())
        .expect("trimmed face");
    let m = creation::measure(&brep, &limits()).expect("measure");
    let want = 100.0 - std::f64::consts::PI * 4.0;
    assert!((m.area_mm2 - want).abs() < 1e-6, "{} vs {want}", m.area_mm2);
    assert_eq!(m.faces, 1);
}

#[test]
fn a_round_face_on_a_plane_and_a_whole_surface() {
    // A disc: the circle as the only (outer) loop, either winding.
    for reversed in [false, true] {
        let brep = creation::trimmed_face(&sheet(), &[vec![circle([5., 5., 0.], 3., reversed)]], DEFAULT_DEFLECTION_MM, &limits())
            .expect("disc");
        let m = creation::measure(&brep, &limits()).expect("measure");
        assert!((m.area_mm2 - std::f64::consts::PI * 9.0).abs() < 1e-6, "{}", m.area_mm2);
    }
    // No loops: the whole sheet.
    let brep = creation::trimmed_face(&sheet(), &[], DEFAULT_DEFLECTION_MM, &limits()).expect("whole");
    let m = creation::measure(&brep, &limits()).expect("measure");
    assert!((m.area_mm2 - 144.0).abs() < 1e-9, "{}", m.area_mm2);
}

#[test]
fn an_open_loop_is_refused() {
    let open = vec![line([0., 0., 0.], [10., 0., 0.]), line([10., 0., 0.], [10., 10., 0.])];
    assert!(creation::trimmed_face(&sheet(), &[open], DEFAULT_DEFLECTION_MM, &limits()).is_err());
}

#[test]
fn any_winding_of_the_outer_loop_and_of_several_holes_gives_the_same_face() {
    // Outer square drawn clockwise, two holes with opposite windings: OCCT orientation is
    // fixed by the bridge, so the user's drawing direction never matters (as in Rhino 5).
    let outer = vec![
        line([0., 0., 0.], [0., 10., 0.]),
        line([0., 10., 0.], [10., 10., 0.]),
        line([10., 10., 0.], [10., 0., 0.]),
        line([10., 0., 0.], [0., 0., 0.]),
    ];
    let a = vec![circle([3., 3., 0.], 1., false)];
    let b = vec![circle([7., 7., 0.], 1.5, true)];
    let brep = creation::trimmed_face(&sheet(), &[outer, a, b], DEFAULT_DEFLECTION_MM, &limits())
        .expect("two holes");
    let m = creation::measure(&brep, &limits()).expect("measure");
    let want = 100.0 - std::f64::consts::PI * (1.0 + 2.25);
    assert!((m.area_mm2 - want).abs() < 1e-6, "{} vs {want}", m.area_mm2);
    assert_eq!(m.faces, 1);
}
