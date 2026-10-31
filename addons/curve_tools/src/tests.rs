// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use om9_addon_geom::eval_geo;
use om9_addon_geom::mesh::{Mesh, cuboid, sphere};
use serde_json::Value;

fn event(command: &str, input: Input, selection: Vec<Selected>) -> Event {
    Event {
        command: command.into(),
        input,
        state: Value::Null,
        plane: Plane::default(),
        selection,
        pick: None,
        settings: serde_json::from_value(json!({})).unwrap(),
    }
}

fn run(command: &str, selection: Vec<Selected>, inputs: Vec<Input>) -> Reply {
    let mut e = event(command, Input::Start, selection);
    let mut r = CurveTools::step(&e).unwrap();
    for i in inputs {
        e.state = r.state.clone();
        e.input = i;
        r = CurveTools::step(&e).unwrap();
        assert!(r.error.is_none(), "{command}: {:?} ({})", r.error, r.prompt);
    }
    assert!(r.done, "{command} did not finish: {}", r.prompt);
    r
}

fn pt(p: Vec3) -> Input {
    Input::Point { point: p }
}
fn num(v: f64) -> Input {
    Input::Text { text: fmt(v) }
}

fn bbox(p: &[Vec3]) -> [Vec3; 2] {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for q in p {
        for i in 0..3 {
            lo[i] = lo[i].min(q[i]);
            hi[i] = hi[i].max(q[i]);
        }
    }
    [lo, hi]
}

fn curve(id: &str, geo: Geo, closed: bool) -> Selected {
    let points = nurbs::samples(&geo, 200);
    let mut points = points;
    if closed && dist(points[0], *points.last().unwrap()) < 1e-9 {
        points.pop();
    }
    Selected {
        id: id.into(),
        kind: "curve".into(),
        name: id.into(),
        bbox: bbox(&points),
        points,
        closed,
        exact: Some(geo),
        ..Default::default()
    }
}

fn line(id: &str, a: Vec3, b: Vec3) -> Selected {
    curve(id, Geo::Line { start: a, end: b }, false)
}

fn circle(id: &str, c: Vec3, r: f64) -> Selected {
    curve(
        id,
        Geo::Circle {
            center: c,
            radius: r,
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
        },
        true,
    )
}

fn solid(id: &str, m: Mesh) -> Selected {
    Selected {
        id: id.into(),
        kind: "mesh".into(),
        name: id.into(),
        bbox: m.bbox(),
        mesh: Some(SelectedMesh {
            vertices: m.vertices,
            triangles: m.triangles,
        }),
        ..Default::default()
    }
}

fn length(g: &Geo) -> f64 {
    if let Geo::Polyline { points, closed } = g {
        let mut l: f64 = points.windows(2).map(|w| dist(w[0], w[1])).sum();
        if *closed {
            l += dist(points[0], *points.last().unwrap());
        }
        return l;
    }
    let p = nurbs::samples(g, 2000);
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

#[test]
fn rebuild_and_refit_keep_the_shape() {
    let c = circle("c", [0.0; 3], 5.0);
    let r = run(REBUILD, vec![c.clone()], vec![num(12.0), num(3.0)]);
    let g = &r.replace[0].geo;
    for p in nurbs::samples(g, 100) {
        assert!((norm(p) - 5.0).abs() < 0.05, "{p:?}");
    }
    let r = run(REFIT, vec![c], vec![num(0.001)]);
    for p in nurbs::samples(&r.replace[0].geo, 300) {
        assert!((norm(p) - 5.0).abs() < 0.002, "{}", norm(p));
    }
}

fn enter(command: &str, mut r: Reply, picks: &[(Selected, Vec3)]) -> Reply {
    let mut e = event(command, Input::Enter, picks.iter().map(|p| p.0.clone()).collect());
    e.state = r.state.clone();
    e.pick = picks.last().map(|p| p.1);
    r = CurveTools::step(&e).unwrap();
    assert!(r.error.is_none(), "{command}: {:?}", r.error);
    r
}

#[test]
fn blends_leave_the_picked_ends_tangentially() {
    let a = line("a", [-10.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    let b = line("b", [5.0, 5.0, 0.0], [5.0, 15.0, 0.0]);
    let picks = [(a.clone(), [-1.0, 0.0, 0.0]), (b.clone(), [5.0, 6.0, 0.0])];
    for cmd in [BLEND, ARC_BLEND, BLEND_ADJ] {
        let mut r = picking(cmd, &[], &picks);
        if cmd != BLEND {
            assert!(!r.done && r.want == Want::Nothing, "{cmd}: {}", r.prompt);
            assert!(!r.preview.is_empty(), "{cmd} previews");
            r = enter(cmd, r, &picks);
        }
        assert!(r.done, "{cmd}: {}", r.prompt);
        let mut p = Vec::new();
        for g in &r.create {
            p.extend(nurbs::samples(g, 400));
        }
        assert!(dist(p[0], [0.0; 3]) < 1e-6, "{cmd} starts at the end of a: {:?}", p[0]);
        assert!(dist(*p.last().unwrap(), [5.0, 5.0, 0.0]) < 1e-6, "{cmd}");
        let n = p.len();
        let t0 = unit(sub(p[1], p[0]));
        let t1 = unit(sub(p[n - 1], p[n - 2]));
        assert!(dot(t0, [1.0, 0.0, 0.0]) > 0.999, "{cmd} leaves along +x: {t0:?}");
        assert!(dot(t1, [0.0, 1.0, 0.0]) > 0.999, "{cmd} arrives along +y: {t1:?}");
    }
    // Picking the far ends blends those instead.
    let r = picking(BLEND, &[], &[(a.clone(), [-9.0, 0.0, 0.0]), (b.clone(), [5.0, 14.0, 0.0])]);
    let (p, q) = ends_of(&r.create[0]);
    assert!(dist(p, [-10.0, 0.0, 0.0]) < 1e-6 && dist(q, [5.0, 15.0, 0.0]) < 1e-6, "{p:?} {q:?}");
    // Adjustable blend with Join: one curve.
    let r = picking(BLEND_ADJ, &[("Join", "Yes"), ("Continuity1", "Tangency")], &picks);
    let r = enter(BLEND_ADJ, r, &picks);
    assert!(r.create.is_empty() && r.delete == ["b"] && r.replace.len() == 1, "{r:?}");
    // Arc blend: two arcs, equal radii by default, a difference when asked.
    // (Ends on one circle make both arcs that circle: use ends that need two.)
    let b2 = line("b2", [7.0, 4.0, 0.0], [7.0, 14.0, 0.0]);
    let picks = [(a.clone(), [-1.0, 0.0, 0.0]), (b2, [7.0, 5.0, 0.0])];
    let r = picking(ARC_BLEND, &[("RadiusDifference", "2")], &picks);
    let r = enter(ARC_BLEND, r, &picks);
    let radii: Vec<f64> = r.create.iter().map(|g| match g { Geo::Arc { radius, .. } => *radius, _ => f64::NAN }).collect();
    assert!((radii[0] - radii[1] - 2.0).abs() < 1e-6, "{radii:?}");
}

#[test]
fn match_moves_the_picked_end_onto_the_other_curve() {
    let a = line("a", [-10.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    let arc = curve("b", Geo::Arc { center: [3.0, 6.0, 0.0], x: [-1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], radius: 2.0, angle: 1.5 }, false);
    let picks = [(a.clone(), [-1.0, 0.0, 0.0]), (arc.clone(), eval_geo(&arc.exact.clone().unwrap(), 0.05).unwrap())];
    let r = picking(MATCH, &[], &picks);
    assert_eq!(r.options.iter().map(|o| o.name.as_str()).filter(|n| *n != "Undo").collect::<Vec<_>>(), ["Continuity", "PreserveOtherEnd", "Join"]);
    let r = enter(MATCH, r, &picks);
    let g = replaced(&r, "a");
    let (p, q) = ends_of(g);
    assert!(dist(p, [-10.0, 0.0, 0.0]) < 1e-3, "other end kept: {p:?}");
    assert!(dist(q, [1.0, 6.0, 0.0]) < 1e-9, "{q:?}");
    // Tangent and curvature continue the arc (leaving its start backwards: -y).
    let c = om9_addon_geom::pick::CurveRef::of(&curve("m", g.clone(), false)).unwrap();
    let end = om9_addon_geom::blend::end_of(&c, 1.0, true);
    assert!(dist(end.d[0], [0.0, 1.0, 0.0]) < 1e-4, "{:?}", end.d[0]);
    assert!(dist(end.d[1], [0.5, 0.0, 0.0]) < 2e-2, "{:?}", end.d[1]);
}

#[test]
fn offsets_keep_their_distance() {
    let c = circle("c", [0.0; 3], 5.0);
    let r = run(OFFSET, vec![c.clone()], vec![num(1.0), pt([10.0, 0.0, 0.0])]);
    assert!(matches!(r.create[0], Geo::Circle { radius, .. } if (radius - 6.0).abs() < 1e-12), "outside: {:?}", r.create[0]);
    let r = run(OFFSET, vec![c], vec![num(1.0), pt([0.5, 0.0, 0.0])]);
    assert!(matches!(r.create[0], Geo::Circle { radius, .. } if (radius - 4.0).abs() < 1e-12), "inside");
    let l = line("l", [0.0; 3], [10.0, 0.0, 0.0]);
    let r = run(OFFSET, vec![l], vec![num(2.0), pt([5.0, 3.0, 0.0])]);
    assert_eq!(r.create[0], Geo::Line { start: [0.0, 2.0, 0.0], end: [10.0, 2.0, 0.0] });
}

#[test]
fn offset_prompts_and_polyline_corners() {
    let r = picking(OFFSET, &[], &[]);
    assert_eq!(r.prompt, "Select curve to offset");
    let names: Vec<String> = r.options.iter().map(|o| format!("{}={}", o.name, o.value.clone().unwrap_or_default())).collect();
    assert_eq!(names, ["Distance=1", "Corner=Sharp", "ThroughPoint=No", "Tolerance=0.001", "BothSides=No", "InCPlane=No", "Cap=None"]);
    // An open L: outside corner at (10, 0) when offsetting below/right.
    let poly = curve("p", Geo::Polyline { points: vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], closed: false }, false);
    let side = [5.0, -3.0, 0.0];
    let go = |opts: &[(&str, &str)]| {
        let r = picking(OFFSET, opts, &[(poly.clone(), [5.0, 0.0, 0.0])]);
        assert_eq!(r.prompt, "Side to offset");
        let mut e = event(OFFSET, pt(side), vec![poly.clone()]);
        e.state = r.state.clone();
        let r = CurveTools::step(&e).unwrap();
        assert!(r.done, "{:?}", r.error);
        r.create
    };
    let sharp = go(&[("Distance", "2")]);
    assert!(matches!(&sharp[0], Geo::Polyline { points, .. } if points == &vec![[0.0, -2.0, 0.0], [12.0, -2.0, 0.0], [12.0, 10.0, 0.0]]), "{sharp:?}");
    let round = go(&[("Distance", "2"), ("Corner", "Round")]);
    let rc = om9_addon_geom::pick::CurveRef::of(&curve("r", round[0].clone(), false)).unwrap();
    assert!((dist(rc.closest([12.0, -2.0, 0.0]).1, [10.0, 0.0, 0.0]) - 2.0).abs() < 1e-6, "round corner at distance 2");
    assert!((length(&round[0]) - (10.0 + 10.0 + std::f64::consts::PI)).abs() < 1e-2, "{}", length(&round[0]));
    let chamfer = go(&[("Distance", "2"), ("Corner", "Chamfer")]);
    assert!((length(&chamfer[0]) - (20.0 + 8f64.sqrt())).abs() < 1e-9, "{}", length(&chamfer[0]));
    let smooth = go(&[("Distance", "2"), ("Corner", "Smooth")]);
    assert!(length(&smooth[0]) > 20.0 && length(&smooth[0]) < 24.0);
    // Inside a corner the segments are trimmed whatever the corner style.
    let mut e = event(OFFSET, Input::Start, vec![]);
    let mut rr = picking(OFFSET, &[("Corner", "Round")], &[(poly.clone(), [5.0, 0.0, 0.0])]);
    e.selection = vec![poly.clone()];
    e.state = rr.state.clone();
    e.input = pt([5.0, 3.0, 0.0]);
    rr = CurveTools::step(&e).unwrap();
    assert!(matches!(&rr.create[0], Geo::Polyline { points, .. } if dist(points[1], [9.0, 1.0, 0.0]) < 1e-12), "{:?}", rr.create);
    // BothSides with a flat cap: one closed outline.
    let both = go(&[("BothSides", "Yes"), ("Cap", "Flat")]);
    assert_eq!(both.len(), 1);
    let ends = ends_of(&both[0]);
    assert!(dist(ends.0, ends.1) < 1e-9, "closed: {ends:?}");
}

#[test]
fn offset_of_a_free_curve_stays_within_tolerance() {
    let g = om9_addon_geom::interpolate(&[[0.0; 3], [3.0, 2.0, 0.0], [6.0, -1.0, 0.0], [10.0, 1.0, 0.0]], 3, false).unwrap();
    let c = curve("s", g, false);
    let r = run(OFFSET, vec![c.clone()], vec![num(0.5), pt([3.0, 5.0, 0.0])]);
    let src = om9_addon_geom::pick::CurveRef::of(&c).unwrap();
    for p in nurbs::samples(&r.create[0], 200) {
        let d = dist(src.closest(p).1, p);
        assert!((d - 0.5).abs() < 2e-3, "{d}");
    }
}

/// Run a command that picks curves by clicking: each pick adds the curve to the selection
/// with the click point; `options` are typed first (`Radius=2`).
fn picking(command: &str, options: &[(&str, &str)], picks: &[(Selected, Vec3)]) -> Reply {
    let mut e = event(command, Input::Start, vec![]);
    let mut r = CurveTools::step(&e).unwrap();
    for (name, value) in options {
        e.state = r.state.clone();
        e.input = Input::Option { name: (*name).into(), value: Some((*value).into()) };
        r = CurveTools::step(&e).unwrap();
        assert!(r.error.is_none(), "{command}: {:?}", r.error);
    }
    for (sel, at) in picks {
        e.state = r.state.clone();
        e.selection.push(sel.clone());
        e.pick = Some(*at);
        e.input = Input::Selection;
        r = CurveTools::step(&e).unwrap();
        assert!(r.error.is_none(), "{command}: {:?} ({})", r.error, r.prompt);
    }
    r
}

fn ends_of(g: &Geo) -> (Vec3, Vec3) {
    (eval_geo(g, 0.0).unwrap(), eval_geo(g, 1.0).unwrap())
}

fn replaced<'a>(r: &'a Reply, id: &str) -> &'a Geo {
    &r.replace.iter().find(|x| x.id == id).unwrap_or_else(|| panic!("{id} not replaced")).geo
}

#[test]
fn fillet_keeps_the_picked_sides() {
    let a = line("a", [-10.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    let b = line("b", [0.0, -10.0, 0.0], [0.0, 10.0, 0.0]);
    let r = picking(FILLET, &[], &[]);
    assert_eq!(r.prompt, "Select first curve to fillet");
    let names: Vec<String> = r.options.iter().map(|o| format!("{}={}", o.name, o.value.clone().unwrap_or_default())).collect();
    assert_eq!(names, ["Radius=1", "Join=No", "Trim=Yes", "ExtendArcsBy=Arc"]);
    let r = picking(FILLET, &[], &[(a.clone(), [5.0, 0.0, 0.0])]);
    assert_eq!(r.prompt, "Select second curve to fillet");
    // Picks on +x and +y: the corner in that quadrant, radius 2 around (2, 2).
    let r = picking(FILLET, &[("Radius", "2")], &[(a.clone(), [5.0, 0.0, 0.0]), (b.clone(), [0.0, 6.0, 0.0])]);
    assert!(r.done, "{}", r.prompt);
    for p in nurbs::samples(&r.create[0], 50) {
        assert!((dist(p, [2.0, 2.0, 0.0]) - 2.0).abs() < 1e-6, "{p:?}");
    }
    assert!((length(&r.create[0]) - std::f64::consts::PI).abs() < 1e-3);
    let (p, q) = ends_of(replaced(&r, "a"));
    assert!(dist(p, [2.0, 0.0, 0.0]) < 1e-6 && dist(q, [10.0, 0.0, 0.0]) < 1e-6, "{p:?} {q:?}");
    let (p, q) = ends_of(replaced(&r, "b"));
    assert!(dist(p, [0.0, 2.0, 0.0]) < 1e-6 && dist(q, [0.0, 10.0, 0.0]) < 1e-6, "{p:?} {q:?}");
    // The other quadrant: picks on -x and -y.
    let r = picking(FILLET, &[], &[(a.clone(), [-5.0, 0.0, 0.0]), (b.clone(), [0.0, -6.0, 0.0])]);
    let (p, _) = ends_of(&r.create[0]);
    assert!(dist(p, [-1.0, 0.0, 0.0]) < 1e-6, "{p:?}");
    // Join=Yes: one curve replaces both.
    let r = picking(FILLET, &[("Join", "Yes")], &[(a.clone(), [5.0, 0.0, 0.0]), (b.clone(), [0.0, 6.0, 0.0])]);
    assert!(r.create.is_empty() && r.delete == ["b"], "{r:?}");
    let j = replaced(&r, "a");
    assert!((length(j) - (9.0 + 9.0 + std::f64::consts::FRAC_PI_2)).abs() < 1e-2, "{}", length(j));
    // Trim=No leaves the lines alone.
    let r = picking(FILLET, &[("Trim", "No")], &[(a.clone(), [5.0, 0.0, 0.0]), (b.clone(), [0.0, 6.0, 0.0])]);
    assert!(r.replace.is_empty() && r.create.len() == 1);
}

#[test]
fn fillet_extends_short_lines_and_radius_zero_makes_a_corner() {
    let a = line("a", [3.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    let b = line("b", [0.0, 4.0, 0.0], [0.0, 10.0, 0.0]);
    let r = picking(FILLET, &[], &[(a.clone(), [6.0, 0.0, 0.0]), (b.clone(), [0.0, 6.0, 0.0])]);
    let (p, _) = ends_of(replaced(&r, "a"));
    assert!(dist(p, [1.0, 0.0, 0.0]) < 1e-6, "{p:?}");
    let (p, _) = ends_of(replaced(&r, "b"));
    assert!(dist(p, [0.0, 1.0, 0.0]) < 1e-6, "{p:?}");
    let r = picking(FILLET, &[("Radius", "0")], &[(a.clone(), [6.0, 0.0, 0.0]), (b.clone(), [0.0, 6.0, 0.0])]);
    assert!(r.create.is_empty());
    let (p, _) = ends_of(replaced(&r, "a"));
    assert!(dist(p, [0.0; 3]) < 1e-6, "{p:?}");
    let (p, _) = ends_of(replaced(&r, "b"));
    assert!(dist(p, [0.0; 3]) < 1e-6, "{p:?}");
}

#[test]
fn fillet_a_line_and_a_circle() {
    let a = line("a", [-10.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    let c = circle("c", [0.0, 4.0, 0.0], 1.0);
    let r = picking(FILLET, &[("Radius", "2")], &[(a, [5.0, 0.0, 0.0]), (c, [1.0, 4.0, 0.0])]);
    assert!(r.done, "{:?} {}", r.error, r.prompt);
    let center = [5f64.sqrt(), 2.0, 0.0];
    for p in nurbs::samples(&r.create[0], 30) {
        assert!((dist(p, center) - 2.0).abs() < 1e-5, "{p:?}");
    }
    let (p, q) = ends_of(&r.create[0]);
    let on_line = if p[1].abs() < 1e-6 { p } else { q };
    assert!(dist(on_line, [5f64.sqrt(), 0.0, 0.0]) < 1e-5, "{p:?} {q:?}");
}

#[test]
fn chamfer_measures_from_the_corner() {
    let a = line("a", [-10.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    let b = line("b", [0.0, 0.0, 0.0], [0.0, 10.0, 0.0]);
    let r = picking(CHAMFER, &[], &[]);
    assert_eq!(r.prompt, "Select first curve to chamfer");
    assert_eq!(r.options[0].value.as_deref(), Some("1,1"));
    let r = picking(CHAMFER, &[("Distances", "1,3")], &[(a, [-5.0, 0.0, 0.0]), (b, [0.0, 5.0, 0.0])]);
    let (p, q) = ends_of(&r.create[0]);
    assert!(dist(p, [-1.0, 0.0, 0.0]) < 1e-9 && dist(q, [0.0, 3.0, 0.0]) < 1e-9, "{p:?} {q:?}");
    let (_, q) = ends_of(replaced(&r, "a"));
    assert!(dist(q, [-1.0, 0.0, 0.0]) < 1e-9, "{q:?}");
}

#[test]
fn clicking_a_number_option_asks_for_it() {
    let mut e = event(FILLET, Input::Start, vec![]);
    let mut r = CurveTools::step(&e).unwrap();
    e.state = r.state.clone();
    e.input = Input::Option { name: "Radius".into(), value: None };
    r = CurveTools::step(&e).unwrap();
    assert_eq!(r.prompt, "Radius <1>");
    e.state = r.state.clone();
    e.input = num(0.5);
    r = CurveTools::step(&e).unwrap();
    assert_eq!(r.prompt, "Select first curve to fillet");
    assert_eq!(r.options[0].value.as_deref(), Some("0.5"));
    // A number typed while picking sets Radius too (Rhino habit).
    e.state = r.state.clone();
    e.input = num(3.0);
    r = CurveTools::step(&e).unwrap();
    assert_eq!(r.options[0].value.as_deref(), Some("3"));
}

#[test]
fn fillet_corners_rounds_a_square() {
    let sq = vec![
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 10.0, 0.0],
        [0.0, 10.0, 0.0],
    ];
    let mut pts = sq.clone();
    pts.push(sq[0]);
    let g = om9_addon_geom::control_curve(&pts, 1).unwrap();
    let s = curve("sq", g, true);
    let r = run(FILLET_CORNERS, vec![s], vec![num(1.0)]);
    // Perimeter: 4·(10 − 2) + 2π·1.
    let l = length(&r.replace[0].geo);
    assert!(
        (l - (32.0 + 2.0 * std::f64::consts::PI)).abs() < 1e-2,
        "{l}"
    );
}

/// One Extend step.
fn ext_step(r: &Reply, input: Input, sel: Vec<Selected>, pick: Option<Vec3>) -> Reply {
    let mut e = event(EXTEND, input, sel);
    e.state = r.state.clone();
    e.pick = pick;
    let r = CurveTools::step(&e).unwrap();
    assert!(r.error.is_none(), "{:?} ({})", r.error, r.prompt);
    r
}

#[test]
fn extend_by_length_to_boundaries_and_dynamically() {
    let l = line("l", [0.0; 3], [10.0, 0.0, 0.0]);
    let start = CurveTools::step(&event(EXTEND, Input::Start, vec![])).unwrap();
    assert_eq!(start.prompt, "Select boundary objects or enter extension length. Press Enter for dynamic extend");
    assert_eq!(start.options[0].value.as_deref(), Some("Natural"));
    // A length, then a pick near the start: the start grows.
    let r = ext_step(&start, num(2.0), vec![], None);
    assert_eq!(r.prompt, "Select curve to extend");
    let r = ext_step(&r, Input::Selection, vec![l.clone()], Some([1.0, 0.0, 0.0]));
    let r = ext_step(&r, Input::Enter, vec![l.clone()], None);
    assert!(r.done);
    let (p, q) = ends_of(&r.replace[0].geo);
    assert!(dist(p, [-2.0, 0.0, 0.0]) < 1e-9 && dist(q, [10.0, 0.0, 0.0]) < 1e-9, "{p:?} {q:?}");
    // A boundary circle: the end runs to it.
    let b = circle("b", [15.0, 0.0, 0.0], 2.0);
    let r = ext_step(&start, Input::Selection, vec![b.clone()], Some([13.0, 0.0, 0.0]));
    let r = ext_step(&r, Input::Enter, vec![b.clone()], None);
    assert_eq!(r.prompt, "Select curve to extend");
    let r = ext_step(&r, Input::Selection, vec![b.clone(), l.clone()], Some([9.0, 0.0, 0.0]));
    let r = ext_step(&r, Input::Enter, vec![b.clone(), l.clone()], None);
    let (_, q) = ends_of(&r.replace[0].geo);
    assert!(dist(q, [13.0, 0.0, 0.0]) < 1e-6, "{q:?}");
    // An arc keeps its circle with Type=Natural, up to a boundary line.
    let arc = curve("a", Geo::Arc { center: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], radius: 5.0, angle: 1.0 }, false);
    let wall = line("w", [-10.0, -5.0, 0.0], [10.0, 15.0, 0.0]);
    let r = ext_step(&start, Input::Selection, vec![wall.clone()], Some([0.0, 10.0, 0.0]));
    let r = ext_step(&r, Input::Enter, vec![wall.clone()], None);
    let r = ext_step(&r, Input::Selection, vec![wall.clone(), arc.clone()], Some([5.0 * 0.9f64.cos(), 5.0 * 0.9f64.sin(), 0.0]));
    let r = ext_step(&r, Input::Enter, vec![wall.clone(), arc.clone()], None);
    let g = &r.replace[0].geo;
    assert!(matches!(g, Geo::Arc { radius, .. } if (*radius - 5.0).abs() < 1e-12), "{g:?}");
    let (_, q) = ends_of(g);
    assert!(dist(q, [0.0, 5.0, 0.0]) < 1e-6, "first crossing with y = x + 5: {q:?}");
    // Dynamic: Enter with nothing, pick the curve, then the end point.
    let r = ext_step(&start, Input::Enter, vec![], None);
    let r = ext_step(&r, Input::Selection, vec![l.clone()], Some([9.0, 0.0, 0.0]));
    assert_eq!(r.prompt, "End of extension");
    let r = ext_step(&r, pt([14.0, 1.0, 0.0]), vec![l.clone()], None);
    let r = ext_step(&r, Input::Enter, vec![l.clone()], None);
    let (_, q) = ends_of(&r.replace[0].geo);
    assert!(dist(q, [14.0, 0.0, 0.0]) < 1e-6, "{q:?}");
}

#[test]
fn divide_extend_tween_and_seam() {
    let l = line("l", [0.0; 3], [10.0, 0.0, 0.0]);
    let r = run(DIVIDE, vec![l.clone()], vec![num(5.0)]);
    assert_eq!(r.create.len(), 6);
    let a = circle("a", [0.0; 3], 2.0);
    let b = circle("b", [0.0; 3], 4.0);
    let r = run(TWEEN, vec![a.clone(), b], vec![num(1.0)]);
    for p in nurbs::samples(&r.create[0], 50) {
        assert!((norm(p) - 3.0).abs() < 0.02, "{}", norm(p));
    }
    let r = run(SEAM, vec![a], vec![pt([0.0, 3.0, 0.0])]);
    let p = nurbs::samples(&r.replace[0].geo, 10)[0];
    assert!(dist(p, [0.0, 2.0, 0.0]) < 0.05, "{p:?}");
}

#[test]
fn curves_from_solids() {
    let b = solid("b", cuboid([0.0; 3], [2.0, 2.0, 2.0]));
    // An open box (no top): its border is the 8 mm square rim.
    let mut open = cuboid([0.0; 3], [2.0, 2.0, 2.0]);
    let top: Vec<[u32; 3]> = open
        .triangles
        .iter()
        .copied()
        .filter(|t| t.iter().all(|&i| open.vertices[i as usize][2] > 1.0))
        .collect();
    open.triangles.retain(|t| !top.contains(t));
    let r = run(DUP_BORDER, vec![solid("o", open)], vec![Input::Enter]);
    assert_eq!(r.create.len(), 1);
    assert!(
        (length(&r.create[0]) - 8.0).abs() < 1e-6
            || matches!(&r.create[0], Geo::Polyline { closed: true, .. })
    );
    let r = run(DUP_EDGE, vec![b.clone()], vec![Input::Enter]);
    let total: f64 = r.create.iter().map(length).sum();
    assert!((total - 24.0).abs() < 1e-6, "12 edges of 2 mm: {total}");
    let ball = solid("s", sphere(3.0, 48));
    let r = run(SILHOUETTE, vec![ball.clone()], vec![Input::Enter]);
    assert!(!r.create.is_empty());
    let r = run(
        SECTION,
        vec![ball.clone()],
        vec![
            num(1.0),
            num(1.0),
            pt([-5.0, 0.0, 0.0]),
            pt([5.0, 0.0, 0.0]),
        ],
    );
    assert_eq!(r.create.len(), 1);
    let c = solid("c", cuboid([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]));
    let d = solid("d", cuboid([0.0; 3], [2.0, 2.0, 2.0]));
    let r = run(INTERSECTION, vec![c.clone(), d], vec![Input::Enter]);
    let total: f64 = r.create.iter().map(length).sum();
    assert!((total - 6.0).abs() < 1e-2, "{total}");
    // Project a circle down onto the top of the box.
    let top = solid("top", cuboid([-10.0, -10.0, 0.0], [10.0, 10.0, 3.0]));
    let r = run(
        PROJECT,
        vec![circle("k", [0.0; 3], 2.0), top.clone()],
        vec![Input::Enter],
    );
    for p in nurbs::samples(&r.create[0], 30) {
        assert!((p[2] - 3.0).abs() < 1e-6, "{p:?}");
    }
    let r = run(
        PULLBACK,
        vec![circle("k", [0.0, 0.0, 5.0], 2.0), top],
        vec![Input::Enter],
    );
    for p in nurbs::samples(&r.create[0], 30) {
        assert!((p[2] - 3.0).abs() < 1e-3, "{p:?}");
    }
}

#[test]
fn curve_boolean_of_two_squares() {
    let sq = |c: Vec3| {
        let p = vec![
            add(c, [-1.0, -1.0, 0.0]),
            add(c, [1.0, -1.0, 0.0]),
            add(c, [1.0, 1.0, 0.0]),
            add(c, [-1.0, 1.0, 0.0]),
            add(c, [-1.0, -1.0, 0.0]),
        ];
        om9_addon_geom::control_curve(&p, 1).unwrap()
    };
    let a = curve("a", sq([0.0; 3]), true);
    let b = curve("b", sq([1.0, 0.0, 0.0]), true);
    let r = run(
        CURVE_BOOLEAN,
        vec![a.clone(), b.clone()],
        vec![Input::Enter],
    );
    assert_eq!(r.create.len(), 1);
    // Union outline: 3 × 2 rectangle → perimeter 10.
    let p = match &r.create[0] {
        Geo::Polyline {
            points,
            closed: true,
        } => points.clone(),
        g => panic!("{g:?}"),
    };
    let mut l: f64 = p.windows(2).map(|w| dist(w[0], w[1])).sum();
    l += dist(p[0], *p.last().unwrap());
    assert!((l - 10.0).abs() < 1e-6, "{l}");
    assert_eq!(r.delete.len(), 2);
}

#[test]
fn clicked_curves_and_two_views() {
    let r = run(
        SKETCH,
        vec![],
        vec![
            pt([0.0; 3]),
            pt([1.0, 1.0, 0.0]),
            pt([2.0, 0.0, 0.0]),
            pt([3.0, 1.0, 0.0]),
            Input::Enter,
        ],
    );
    assert_eq!(r.create.len(), 1);
    let floor = solid("f", cuboid([-10.0, -10.0, -1.0], [10.0, 10.0, 2.0]));
    let r = run(
        POLY_ON_SRF,
        vec![floor],
        vec![pt([0.0; 3]), pt([3.0, 0.0, 0.0]), Input::Enter],
    );
    for p in nurbs::samples(&r.create[0], 10) {
        assert!((p[2] - 2.0).abs() < 1e-9);
    }
    // Top view: a line along x; front view: a slope z = x / 2.
    let top = line("t", [0.0; 3], [10.0, 0.0, 0.0]);
    let front = curve(
        "f",
        Geo::Line {
            start: [0.0, 0.0, 0.0],
            end: [10.0, 0.0, 5.0],
        },
        false,
    );
    let r = run(CRV2VIEW, vec![top, front], vec![Input::Enter]);
    let p = nurbs::samples(&r.create[0], 10);
    assert!(dist(*p.last().unwrap(), [10.0, 0.0, 5.0]) < 1e-6, "{p:?}");
}

#[test]
fn soft_edit_moves_only_near_the_pick() {
    let l = line("l", [0.0; 3], [20.0, 0.0, 0.0]);
    let r = run(
        SOFT_EDIT,
        vec![l],
        vec![num(4.0), pt([10.0, 0.0, 0.0]), pt([10.0, 3.0, 0.0])],
    );
    let g = &r.replace[0].geo;
    let p = nurbs::samples(g, 400);
    let peak = p.iter().map(|q| q[1]).fold(f64::MIN, f64::max);
    assert!((peak - 3.0).abs() < 0.05, "{peak}");
    assert!(p[0][1].abs() < 1e-6 && p[399][1].abs() < 1e-6);
}

#[test]
fn every_command_answers_start() {
    for c in CurveTools::SESSION_COMMANDS {
        let r = CurveTools::step(&event(c, Input::Start, vec![])).unwrap();
        assert!(!r.done, "{c}");
        assert!(!r.prompt.is_empty(), "{c}");
    }
}
