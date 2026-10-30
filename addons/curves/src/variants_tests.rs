// SPDX-License-Identifier: LGPL-2.1-or-later
//! The Matrix 9 sub-options of Polyline, InterpCrv, Rectangle, Circle and Ellipse, step by
//! step as OM9 sends them, with the geometry they create.
use crate::*;
use om9_addon_abi::session::{Selected, dist, norm};

fn ev(command: &str, input: Input, state: &Value, sel: &[Selected]) -> Event {
    let mut e = event(command, input, state);
    e.selection = sel.to_vec();
    e
}

/// Every reply of the run (the first is Start).
fn run_sel(command: &str, inputs: Vec<Input>, sel: &[Selected]) -> Vec<Reply> {
    let mut state = Value::Null;
    let mut out = Vec::new();
    for input in std::iter::once(Input::Start).chain(inputs) {
        let hover = matches!(input, Input::Hover { .. });
        let r = Curves::step(&ev(command, input, &state, sel)).unwrap();
        if !hover {
            state = r.state.clone();
        }
        out.push(r);
    }
    out
}
fn run(command: &str, inputs: Vec<Input>) -> Vec<Reply> {
    run_sel(command, inputs, &[])
}
fn pt(x: f64, y: f64) -> Input {
    Input::Point { point: [x, y, 0.0] }
}
fn opt(name: &str) -> Input {
    Input::Option {
        name: name.into(),
        value: None,
    }
}
fn set(name: &str, v: &str) -> Input {
    Input::Option {
        name: name.into(),
        value: Some(v.into()),
    }
}
fn text(t: &str) -> Input {
    Input::Text { text: t.into() }
}
fn names(r: &Reply) -> Vec<String> {
    r.options
        .iter()
        .map(|o| match &o.value {
            Some(v) => format!("{}={v}", o.name),
            None => o.name.clone(),
        })
        .collect()
}
fn ok(r: &[Reply]) -> &Reply {
    for (i, x) in r.iter().enumerate() {
        assert!(x.error.is_none(), "step {i}: {:?} ({})", x.error, x.prompt);
    }
    r.last().unwrap()
}
fn created(r: &[Reply]) -> Geo {
    let last = ok(r);
    assert!(last.done, "{}", last.prompt);
    last.create[0].clone()
}
fn at(g: &Geo, t: f64) -> Vec3 {
    g::eval_geo(g, t).unwrap()
}
fn tangent(g: &Geo, t: f64) -> Vec3 {
    let h = 1e-6;
    let (a, b) = if t < 0.5 { (t, t + h) } else { (t - h, t) };
    unit(sub(at(g, b), at(g, a)))
}
fn circle_sel(id: &str, cx: f64, cy: f64, r: f64) -> Selected {
    Selected {
        id: id.into(),
        kind: "curve".into(),
        closed: true,
        points: (0..512)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 512.0;
                [cx + r * a.cos(), cy + r * a.sin(), 0.0]
            })
            .collect(),
        exact: Some(Geo::Circle {
            center: [cx, cy, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            radius: r,
        }),
        ..Default::default()
    }
}

#[test]
fn polyline_prompts_follow_matrix() {
    let r = run(POLYLINE, vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0)]);
    assert_eq!(r[0].prompt, "Start of polyline");
    assert_eq!(names(&r[0]), ["PersistentClose=No"]);
    assert_eq!(r[1].prompt, "Next point of polyline");
    assert_eq!(
        names(&r[1]),
        ["PersistentClose=No", "Mode=Line", "Helpers=No", "Undo"]
    );
    assert_eq!(r[2].prompt, "Next point of polyline. Press Enter when done");
    assert_eq!(
        names(&r[2]),
        [
            "PersistentClose=No",
            "Mode=Line",
            "Helpers=No",
            "Length",
            "Undo"
        ]
    );
    assert_eq!(
        names(&r[3]),
        [
            "PersistentClose=No",
            "Close",
            "Mode=Line",
            "Helpers=No",
            "Length",
            "Undo"
        ]
    );
}

#[test]
fn polyline_arc_mode_is_tangent_and_exact() {
    // A line, then an arc tangent to it ending at (20, 10): a half circle of radius 5.
    let g = created(&run(
        POLYLINE,
        vec![
            pt(0.0, 0.0),
            pt(10.0, 0.0),
            opt("Mode"),
            pt(10.0, 10.0),
            Input::Enter,
        ],
    ));
    let Geo::Nurbs { degree: 2, .. } = g else {
        panic!("{g:?}")
    };
    assert!(dist(at(&g, 0.0), [0.0; 3]) < 1e-9);
    assert!(dist(at(&g, 1.0), [10.0, 10.0, 0.0]) < 1e-9);
    // Every sample of the arc part is 5 from its center (10, 5).
    for i in 0..=40 {
        let p = at(&g, 0.5 + 0.5 * i as f64 / 40.0);
        if p[0] > 10.0 + 1e-6 {
            assert!((dist(p, [10.0, 5.0, 0.0]) - 5.0).abs() < 1e-9, "{p:?}");
        }
    }
    // Arc mode shows Direction and Center instead of Length; Center makes the arc around it.
    let r = run(POLYLINE, vec![pt(0.0, 0.0), set("Mode", "Arc")]);
    assert!(names(r.last().unwrap()).contains(&"Direction".into()));
    let g = created(&run(
        POLYLINE,
        vec![
            pt(5.0, 0.0),
            set("Mode", "Arc"),
            opt("Center"),
            pt(0.0, 0.0),
            pt(0.0, 9.0),
            Input::Enter,
        ],
    ));
    assert!(
        dist(at(&g, 1.0), [0.0, 5.0, 0.0]) < 1e-9,
        "{:?}",
        at(&g, 1.0)
    );
    // Helpers draw guide lines.
    let r = run(
        POLYLINE,
        vec![
            pt(0.0, 0.0),
            opt("Helpers"),
            Input::Hover {
                point: [3.0, 1.0, 0.0],
            },
        ],
    );
    assert!(r.last().unwrap().preview.len() >= 3);
}

#[test]
fn interp_knots_submenu_tangents_and_closing() {
    let r = run(INTERP, vec![]);
    assert_eq!(r[0].prompt, "Start of curve");
    assert_eq!(
        names(&r[0]),
        [
            "Degree=3",
            "Knots=Uniform",
            "PersistentClose=No",
            "StartTangent"
        ]
    );
    // Knots opens its own choice.
    let r = run(INTERP, vec![opt("Knots")]);
    assert_eq!(r[1].prompt, "Knots <Uniform>");
    assert_eq!(names(&r[1]), ["Uniform", "Chord", "SqrtChord"]);
    let r = run(INTERP, vec![opt("Knots"), opt("Chord")]);
    assert!(names(&r[2]).contains(&"Knots=Chord".into()));
    let r = run(INTERP, vec![set("Knots", "SqrtChrd")]);
    assert!(names(&r[1]).contains(&"Knots=SqrtChord".into()));
    // After three points: Close and Sharp.
    let pts = || vec![pt(0.0, 0.0), pt(10.0, 3.0), pt(12.0, 12.0), pt(2.0, 14.0)];
    let mut inputs = pts();
    inputs.truncate(3);
    let r = run(INTERP, inputs);
    for n in ["Close", "Sharp", "EndTangent", "Undo"] {
        assert!(names(&r[3]).contains(&n.to_owned()), "{n}");
    }
    // Close: smooth seam; Sharp: kinked seam.
    let mut c = pts();
    c.push(opt("Close"));
    let g = created(&run(INTERP, c));
    assert!(dist(at(&g, 0.0), at(&g, 1.0)) < 1e-9);
    assert!(dist(tangent(&g, 0.0), tangent(&g, 1.0)) < 1e-4);
    let mut s = pts();
    s.push(opt("Sharp"));
    let g = created(&run(INTERP, s));
    assert!(dist(tangent(&g, 0.0), tangent(&g, 1.0)) > 1e-2);
    // Picking the start again closes smoothly.
    let mut a = pts();
    a.push(pt(0.0, 0.0));
    let g = created(&run(INTERP, a));
    assert!(dist(tangent(&g, 0.0), tangent(&g, 1.0)) < 1e-4);
    // Start and end tangents.
    let g = created(&run(
        INTERP,
        vec![
            opt("StartTangent"),
            pt(0.0, 0.0),
            pt(0.0, 5.0),
            pt(10.0, 3.0),
            pt(12.0, 12.0),
            opt("EndTangent"),
            pt(20.0, 12.0),
            pt(30.0, 12.0),
        ],
    ));
    assert!(
        dist(tangent(&g, 0.0), [0.0, 1.0, 0.0]) < 1e-4,
        "{:?}",
        tangent(&g, 0.0)
    );
    assert!(
        dist(tangent(&g, 1.0), [1.0, 0.0, 0.0]) < 1e-4,
        "{:?}",
        tangent(&g, 1.0)
    );
    assert!(dist(at(&g, 1.0), [20.0, 12.0, 0.0]) < 1e-9);
}

#[test]
fn rectangle_rounded_arc_conic_and_pass_through() {
    let r = run(RECTANGLE, vec![]);
    assert_eq!(names(&r[0]), ["3Point", "Vertical", "Center", "Rounded=No"]);
    let r = run(RECTANGLE, vec![opt("Rounded"), pt(0.0, 0.0), pt(10.0, 6.0)]);
    assert_eq!(
        r[3].prompt,
        "Radius or point for rounded corner to pass through <0.6>"
    );
    assert_eq!(names(&r[3]), ["Corner=Arc"]);
    // Arc corners: the corner arc's middle is r(1 − 1/√2) inside the corner on the diagonal.
    let g = created(&run(
        RECTANGLE,
        vec![opt("Rounded"), pt(0.0, 0.0), pt(10.0, 6.0), text("2")],
    ));
    let d = (0..=2000)
        .map(|i| dist(at(&g, i as f64 / 2000.0), [0.0, 0.0, 0.0]))
        .fold(f64::INFINITY, f64::min);
    assert!((d - 2.0 * (2f64.sqrt() - 1.0)).abs() < 1e-3, "{d}");
    // Conic corners with Rho: a different (flatter for rho > 1 − 1/√2) corner.
    let r = run(
        RECTANGLE,
        vec![
            opt("Rounded"),
            pt(0.0, 0.0),
            pt(10.0, 6.0),
            set("Corner", "Conic"),
        ],
    );
    assert_eq!(names(&r[4]), ["Corner=Conic", "Rho=0.5"]);
    let g = created(&run(
        RECTANGLE,
        vec![
            opt("Rounded"),
            pt(0.0, 0.0),
            pt(10.0, 6.0),
            set("Corner", "Conic"),
            set("Rho", "0.8"),
            text("2"),
        ],
    ));
    let dc = (0..=2000)
        .map(|i| dist(at(&g, i as f64 / 2000.0), [0.0, 0.0, 0.0]))
        .fold(f64::INFINITY, f64::min);
    assert!(dc < d - 0.1, "rho 0.8 hugs the corner: {dc} vs {d}");
    // A picked point: the corner passes through it.
    let g = created(&run(
        RECTANGLE,
        vec![opt("Rounded"), pt(0.0, 0.0), pt(10.0, 6.0), pt(0.3, 0.3)],
    ));
    let dp = (0..=4000)
        .map(|i| dist(at(&g, i as f64 / 4000.0), [0.3, 0.3, 0.0]))
        .fold(f64::INFINITY, f64::min);
    assert!(dp < 1e-2, "{dp}");
    // Length, then Enter: a square.
    let Geo::Polyline { points, .. } =
        created(&run(RECTANGLE, vec![pt(0.0, 0.0), text("4"), Input::Enter]))
    else {
        panic!()
    };
    assert_eq!(points[2], [4.0, 4.0, 0.0]);
}

#[test]
fn circle_sub_options() {
    let r = run(CIRCLE, vec![]);
    assert_eq!(
        names(&r[0]),
        [
            "Deformable",
            "Vertical",
            "2Point",
            "3Point",
            "Tangent",
            "AroundCurve",
            "FitPoints"
        ]
    );
    let r = run(CIRCLE, vec![pt(0.0, 0.0)]);
    assert_eq!(r[1].prompt, "Radius");
    assert_eq!(
        names(&r[1]),
        ["Diameter", "Orientation", "Circumference", "Area"]
    );
    let r = run(CIRCLE, vec![pt(0.0, 0.0), opt("Diameter")]);
    assert_eq!(r[2].prompt, "Diameter");
    assert_eq!(
        names(&r[2]),
        ["Radius", "Orientation", "Circumference", "Area"]
    );
    let radius = |g: &Geo| match g {
        Geo::Circle { radius, .. } => *radius,
        _ => panic!("{g:?}"),
    };
    // Circumference and area.
    let g = created(&run(
        CIRCLE,
        vec![pt(0.0, 0.0), opt("Circumference"), text("62.8318530718")],
    ));
    assert!((radius(&g) - 10.0).abs() < 1e-9);
    let g = created(&run(
        CIRCLE,
        vec![pt(0.0, 0.0), opt("Area"), text("314.159265359")],
    ));
    assert!((radius(&g) - 10.0).abs() < 1e-9);
    // Orientation: the circle's normal points at the picked point.
    let r = run(
        CIRCLE,
        vec![
            pt(0.0, 0.0),
            opt("Orientation"),
            Input::Point {
                point: [1.0, 0.0, 0.0],
            },
            text("3"),
        ],
    );
    let Geo::Circle { x, y, .. } = ok(&r).create[0] else {
        panic!()
    };
    assert!(dot(cross(x, y), [1.0, 0.0, 0.0]).abs() > 1.0 - 1e-9);
    // Deformable: Degree / PointCount, a NURBS with 10 + 3 poles (clamped periodic).
    let r = run(CIRCLE, vec![opt("Deformable")]);
    assert_eq!(
        names(&r[1])[..2],
        ["Degree=3".to_owned(), "PointCount=10".to_owned()]
    );
    let g = created(&run(
        CIRCLE,
        vec![opt("Deformable"), pt(0.0, 0.0), text("5")],
    ));
    let Geo::Nurbs {
        points, degree: 3, ..
    } = &g
    else {
        panic!("{g:?}")
    };
    assert_eq!(points.len(), 13);
    for i in 0..10 {
        assert!((norm(at(&g, i as f64 / 10.0)) - 5.0).abs() < 1e-6);
    }
}

#[test]
fn circle_around_curve_tangent_and_fit() {
    // AroundCurve on a line along x: the circle stands in the YZ plane.
    let line = Selected {
        id: "l".into(),
        kind: "curve".into(),
        points: vec![[0.0; 3], [20.0, 0.0, 0.0]],
        exact: Some(Geo::Line {
            start: [0.0; 3],
            end: [20.0, 0.0, 0.0],
        }),
        ..Default::default()
    };
    let r = run_sel(
        CIRCLE,
        vec![opt("AroundCurve"), pt(5.0, 1.0), text("2")],
        std::slice::from_ref(&line),
    );
    let Geo::Circle {
        center,
        x,
        y,
        radius,
    } = ok(&r).create[0]
    else {
        panic!()
    };
    assert!(dist(center, [5.0, 0.0, 0.0]) < 1e-6 && (radius - 2.0).abs() < 1e-12);
    assert!(dot(cross(x, y), [1.0, 0.0, 0.0]).abs() > 1.0 - 1e-6);
    // Tangent to two circles with a radius: centers 20 apart, radius 5 each → r 5 touches
    // both at the midpoint (10, 0) ± … : the circle at (10, ±√(10² − 10²)) … use r = 6.
    let (a, b) = (
        circle_sel("a", 0.0, 0.0, 5.0),
        circle_sel("b", 20.0, 0.0, 5.0),
    );
    let r = run_sel(
        CIRCLE,
        vec![opt("Tangent"), text("6"), pt(10.0, 4.0)],
        &[a.clone(), b.clone()],
    );
    let Geo::Circle { center, radius, .. } = ok(&r).create[0] else {
        panic!()
    };
    assert!((radius - 6.0).abs() < 1e-9);
    // Outside-tangent to both: |c − (0,0)| = 11 → c = (10, √21).
    assert!(dist(center, [10.0, 21f64.sqrt(), 0.0]) < 1e-3, "{center:?}");
    // Three circles: the circle touching all three (outside) found near the pick.
    let c3 = circle_sel("c", 10.0, 20.0, 5.0);
    let r = run_sel(CIRCLE, vec![opt("Tangent"), pt(10.0, 7.0)], &[a, b, c3]);
    let Geo::Circle { center, radius, .. } = ok(&r).create[0] else {
        panic!()
    };
    for (cx, cy) in [(0.0, 0.0), (20.0, 0.0), (10.0, 20.0)] {
        let d = dist(center, [cx, cy, 0.0]);
        assert!((d - (radius + 5.0)).abs() < 1e-3, "{d} {radius}");
    }
    // FitPoints: point objects on a circle of radius 3 around (1, 1).
    let pts: Vec<Selected> = (0..5)
        .map(|i| {
            let a = i as f64;
            let p = [1.0 + 3.0 * a.cos(), 1.0 + 3.0 * a.sin(), 0.0];
            Selected {
                id: format!("p{i}"),
                kind: "point".into(),
                bbox: [p, p],
                ..Default::default()
            }
        })
        .collect();
    let r = run_sel(CIRCLE, vec![opt("FitPoints"), Input::Enter], &pts);
    let Geo::Circle { center, radius, .. } = ok(&r).create[0] else {
        panic!()
    };
    assert!(dist(center, [1.0, 1.0, 0.0]) < 1e-9 && (radius - 3.0).abs() < 1e-9);
}

#[test]
fn ellipse_sub_options() {
    let r = run(ELLIPSE, vec![]);
    assert_eq!(r[0].prompt, "Ellipse center");
    assert_eq!(
        names(&r[0]),
        [
            "Deformable",
            "Vertical",
            "Corner",
            "Diameter",
            "FromFoci",
            "AroundCurve"
        ]
    );
    let r = run(ELLIPSE, vec![pt(0.0, 0.0)]);
    assert_eq!(r[1].prompt, "End of first axis");
    assert_eq!(names(&r[1]), ["Corner"]);
    let semi = |g: &Geo| {
        let xs: Vec<Vec3> = (0..400).map(|i| at(g, i as f64 / 400.0)).collect();
        let c = scale(xs.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / 400.0);
        let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
        for p in &xs {
            lo = lo.min(dist(*p, c));
            hi = hi.max(dist(*p, c));
        }
        (c, hi, lo)
    };
    // Corner: the enclosing rectangle 10 × 4 → semi-axes 5 and 2.
    let (c, a, b) = semi(&created(&run(
        ELLIPSE,
        vec![opt("Corner"), pt(0.0, 0.0), pt(10.0, 4.0)],
    )));
    assert!(dist(c, [5.0, 2.0, 0.0]) < 1e-3 && (a - 5.0).abs() < 1e-3 && (b - 2.0).abs() < 1e-3);
    // Diameter: axis ends (−4, 0)–(4, 0), second axis 1.
    let (c, a, b) = semi(&created(&run(
        ELLIPSE,
        vec![opt("Diameter"), pt(-4.0, 0.0), pt(4.0, 0.0), pt(0.0, 1.0)],
    )));
    assert!(dist(c, [0.0; 3]) < 1e-3 && (a - 4.0).abs() < 1e-3 && (b - 1.0).abs() < 1e-3);
    // FromFoci with MarkFoci: foci (±3, 0), point (5, 0) → a = 5, b = 4, two points marked.
    let r = run(
        ELLIPSE,
        vec![
            opt("FromFoci"),
            opt("MarkFoci"),
            pt(-3.0, 0.0),
            pt(3.0, 0.0),
            pt(5.0, 0.0),
        ],
    );
    let last = ok(&r);
    assert_eq!(last.create.len(), 3, "ellipse + two focus points");
    let (_, a, b) = semi(&last.create[0]);
    assert!((a - 5.0).abs() < 1e-3 && (b - 4.0).abs() < 1e-3, "{a} {b}");
    // Corner after the center: a corner of the box around that center.
    let (c, a, b) = semi(&created(&run(
        ELLIPSE,
        vec![pt(0.0, 0.0), opt("Corner"), pt(6.0, 2.0)],
    )));
    assert!(dist(c, [0.0; 3]) < 1e-3 && (a - 6.0).abs() < 1e-3 && (b - 2.0).abs() < 1e-3);
    // Vertical: the second axis along the CPlane normal.
    let g = created(&run(
        ELLIPSE,
        vec![opt("Vertical"), pt(0.0, 0.0), pt(4.0, 0.0), text("2")],
    ));
    assert!(
        dist(at(&g, 0.25), [0.0, 0.0, 2.0]) < 1e-9,
        "{:?}",
        at(&g, 0.25)
    );
    // Deformable ellipse.
    let g = created(&run(
        ELLIPSE,
        vec![opt("Deformable"), pt(0.0, 0.0), pt(4.0, 0.0), pt(0.0, 2.0)],
    ));
    assert!(matches!(g, Geo::Nurbs { degree: 3, .. }));
}
