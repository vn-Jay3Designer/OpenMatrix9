// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use om9_addon_geom::mesh::{Mesh, cuboid, sphere};
use serde_json::Value;

fn event(command: &str, input: Input, selection: Vec<Selected>) -> Event {
    Event {
        command: command.into(),
        input,
        state: Value::Null,
        plane: Plane::default(),
        selection,
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

#[test]
fn blends_join_the_nearest_ends_tangentially() {
    let a = line("a", [-10.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    let b = line("b", [5.0, 5.0, 0.0], [5.0, 15.0, 0.0]);
    for cmd in [BLEND, ARC_BLEND, BLEND_ADJ] {
        let mut inputs = vec![Input::Enter];
        if cmd == BLEND_ADJ {
            inputs = vec![num(1.0), num(1.0)];
        }
        let r = run(cmd, vec![a.clone(), b.clone()], inputs);
        let p = nurbs::samples(&r.create[0], 400);
        assert!(
            dist(p[0], [0.0; 3]) < 1e-6,
            "{cmd} starts at the end of a: {:?}",
            p[0]
        );
        assert!(dist(*p.last().unwrap(), [5.0, 5.0, 0.0]) < 1e-6, "{cmd}");
        let t0 = unit(sub(p[1], p[0]));
        let t1 = unit(sub(p[399], p[398]));
        assert!(
            dot(t0, [1.0, 0.0, 0.0]) > 0.99,
            "{cmd} leaves along +x: {t0:?}"
        );
        assert!(
            dot(t1, [0.0, 1.0, 0.0]) < -0.99
                || dot(t1, [0.0, -1.0, 0.0]) > 0.99
                || dot(t1, [0.0, 1.0, 0.0]) > 0.99,
            "{cmd} {t1:?}"
        );
    }
}

#[test]
fn offsets_keep_their_distance() {
    let c = circle("c", [0.0; 3], 5.0);
    let r = run(
        OFFSET,
        vec![c.clone()],
        vec![num(1.0), pt([10.0, 0.0, 0.0])],
    );
    for p in nurbs::samples(&r.create[0], 100) {
        assert!((norm(p) - 6.0).abs() < 0.02, "outside: {}", norm(p));
    }
    let r = run(OFFSET, vec![c], vec![num(1.0), pt([0.5, 0.0, 0.0])]);
    for p in nurbs::samples(&r.create[0], 100) {
        assert!((norm(p) - 4.0).abs() < 0.02, "inside: {}", norm(p));
    }
    let l = line("l", [0.0; 3], [10.0, 0.0, 0.0]);
    let r = run(OFFSET, vec![l], vec![num(2.0), pt([5.0, 3.0, 0.0])]);
    for p in nurbs::samples(&r.create[0], 20) {
        assert!((p[1] - 2.0).abs() < 1e-9);
    }
}

#[test]
fn fillet_and_chamfer_meet_two_lines() {
    let a = line("a", [-10.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    let b = line("b", [0.0, 0.0, 0.0], [0.0, 10.0, 0.0]);
    let r = run(FILLET, vec![a.clone(), b.clone()], vec![num(2.0)]);
    // A quarter circle of radius 2 centered at (-2, 2).
    for p in nurbs::samples(&r.create[0], 50) {
        assert!((dist(p, [-2.0, 2.0, 0.0]) - 2.0).abs() < 1e-6, "{p:?}");
    }
    assert!((length(&r.create[0]) - std::f64::consts::PI).abs() < 1e-3);
    assert_eq!(r.replace.len(), 2, "both lines trimmed");
    let ends: Vec<Vec3> = r
        .replace
        .iter()
        .map(|x| *nurbs::samples(&x.geo, 2).last().unwrap())
        .collect();
    let starts: Vec<Vec3> = r
        .replace
        .iter()
        .map(|x| nurbs::samples(&x.geo, 2)[0])
        .collect();
    assert!(dist(ends[0], [-2.0, 0.0, 0.0]) < 1e-6, "{ends:?}");
    assert!(dist(starts[1], [0.0, 2.0, 0.0]) < 1e-6, "{starts:?}");
    let r = run(CHAMFER, vec![a, b], vec![num(1.0), num(3.0)]);
    let p = nurbs::samples(&r.create[0], 2);
    assert!(
        dist(p[0], [-1.0, 0.0, 0.0]) < 1e-9 && dist(*p.last().unwrap(), [0.0, 3.0, 0.0]) < 1e-9,
        "{p:?}"
    );
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

#[test]
fn divide_extend_tween_and_seam() {
    let l = line("l", [0.0; 3], [10.0, 0.0, 0.0]);
    let r = run(DIVIDE, vec![l.clone()], vec![num(5.0)]);
    assert_eq!(r.create.len(), 6);
    let r = run(EXTEND, vec![l.clone()], vec![num(2.0)]);
    let p = nurbs::samples(&r.replace[0].geo, 2);
    assert!(dist(*p.last().unwrap(), [12.0, 0.0, 0.0]) < 1e-9, "{p:?}");
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
