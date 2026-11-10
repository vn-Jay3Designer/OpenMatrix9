// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use std::f64::consts::TAU;
use om9_addon_abi::session::Plane;
use om9_addon_geom::mesh::cuboid;
use serde_json::Value;

fn event(command: &str, input: Input, selection: Vec<Selected>, settings: Value) -> Event {
    Event {
        command: command.into(),
        input,
        state: Value::Null,
        plane: Plane::default(),
        selection,
        pick: None,
        settings: serde_json::from_value(settings).unwrap(),
    }
}

fn run(command: &str, selection: Vec<Selected>, settings: Value, inputs: Vec<Input>) -> Reply {
    let mut e = event(command, Input::Start, selection, settings);
    let mut r = Surfaces::step(&e).unwrap();
    for i in inputs {
        e.state = r.state.clone();
        e.input = i;
        r = Surfaces::step(&e).unwrap();
        assert!(r.error.is_none(), "{command}: {:?} ({})", r.error, r.prompt);
    }
    assert!(r.done, "{command}: {}", r.prompt);
    r
}

fn pt(p: Vec3) -> Input {
    Input::Point { point: p }
}

fn curve(id: &str, points: Vec<Vec3>, closed: bool) -> Selected {
    Selected {
        id: id.into(),
        kind: "curve".into(),
        name: id.into(),
        bbox: [[0.0; 3], [1.0; 3]],
        points,
        closed,
        ..Default::default()
    }
}

fn circle_pts(c: Vec3, r: f64, plane: usize, n: usize) -> Vec<Vec3> {
    (0..n)
        .map(|k| {
            let a = TAU * k as f64 / n as f64;
            match plane {
                0 => add(c, [r * a.cos(), r * a.sin(), 0.0]),
                1 => add(c, [r * a.cos(), 0.0, r * a.sin()]),
                _ => add(c, [0.0, r * a.cos(), r * a.sin()]),
            }
        })
        .collect()
}

fn line(id: &str, a: Vec3, b: Vec3) -> Selected {
    curve(
        id,
        (0..=20)
            .map(|k| add(a, scale(sub(b, a), k as f64 / 20.0)))
            .collect(),
        false,
    )
}

fn srf_sel(id: &str, g: Geo) -> Selected {
    let s = Srf::of(&g).unwrap();
    let m = grid_mesh(&grid(&s, 20, 20));
    Selected {
        id: id.into(),
        kind: "surface".into(),
        name: id.into(),
        bbox: m.bbox(),
        mesh: Some(SelectedMesh {
            vertices: m.vertices,
            triangles: m.triangles,
        }),
        exact: Some(g),
        ..Default::default()
    }
}

fn flat(id: &str, x0: f64, x1: f64, y0: f64, y1: f64, z: f64) -> Selected {
    let rows: Vec<Vec<Vec3>> = (0..4)
        .map(|i| {
            (0..4)
                .map(|j| {
                    [
                        x0 + (x1 - x0) * i as f64 / 3.0,
                        y0 + (y1 - y0) * j as f64 / 3.0,
                        z,
                    ]
                })
                .collect()
        })
        .collect();
    srf_sel(id, surface_on_grid(&rows, [3, 3]).unwrap())
}

#[test]
fn sweep_a_tube_around_a_ring() {
    // Rail: a circle of radius 10 in the XZ plane; profile: a circle of radius 1 around the
    // top of the rail, across it.
    let rail = curve("r", circle_pts([0.0; 3], 10.0, 1, 128), true);
    let prof = curve(
        "p",
        circle_pts([10.0, 0.0, 0.0], 1.0, 0, 64)
            .iter()
            .map(|p| [10.0 + (p[0] - 10.0), p[1], 0.0])
            .map(|p| [p[0], p[1], 0.0])
            .collect(),
        true,
    );
    // The profile stands across the rail at (10, 0, 0): in the plane x-y? the rail runs along
    // z there, so the profile lies in z = 0.
    let r = run(SWEEP1, vec![rail, prof], json!({}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    for i in 0..=20 {
        for j in 0..=20 {
            let p = s.at_st(i as f64 / 20.0, j as f64 / 20.0);
            // Distance from the rail circle.
            let rho = (p[0] * p[0] + p[2] * p[2]).sqrt();
            let d = ((rho - 10.0).powi(2) + p[1] * p[1]).sqrt();
            assert!((d - 1.0).abs() < 0.02, "{p:?} {d}");
        }
    }
    // A window selection also catches the gem layout circle inside the ring: it does not touch
    // the rail, so it is not a profile (found testing Profile Placer + Profile Sweep on Windows).
    let rail = curve("r", circle_pts([0.0; 3], 10.0, 1, 128), true);
    let prof = curve("p", circle_pts([10.0, 0.0, 0.0], 1.0, 0, 64), true);
    let stray = curve("g", circle_pts([0.0; 3], 5.0, 0, 64), true);
    let r = run(
        PROFILE_SWEEP,
        vec![rail, prof, stray],
        json!({}),
        vec![Input::Enter],
    );
    assert!(
        r.messages.iter().any(|m| m.contains("1 profiles")),
        "{:?}",
        r.messages
    );
}

#[test]
fn loft_patch_and_edge_curves() {
    let a = curve("a", circle_pts([0.0; 3], 2.0, 0, 64), true);
    let b = curve("b", circle_pts([0.0, 0.0, 5.0], 3.0, 0, 64), true);
    let r = run(LOFT, vec![a, b], json!({}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    let mid = s.at_st(0.5, 0.3);
    assert!((mid[2] - 2.5).abs() < 0.05, "{mid:?}");
    assert!(((mid[0].powi(2) + mid[1].powi(2)).sqrt() - 2.5).abs() < 0.05);
    // Four edges of a unit square (a flat patch).
    let edges = vec![
        line("1", [0.0; 3], [1.0, 0.0, 0.0]),
        line("2", [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]),
        line("3", [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]),
        line("4", [0.0, 1.0, 0.0], [0.0; 3]),
    ];
    let r = run(EDGE_CURVES, edges, json!({}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    assert!(
        dist(s.at_st(0.5, 0.5), [0.5, 0.5, 0.0]) < 1e-6,
        "{:?}",
        s.at_st(0.5, 0.5)
    );
    // Two rails: a ruled surface.
    let r = run(
        EDGE_CURVES,
        vec![
            line("1", [0.0; 3], [4.0, 0.0, 0.0]),
            line("2", [0.0, 2.0, 1.0], [4.0, 2.0, 1.0]),
        ],
        json!({}),
        vec![Input::Enter],
    );
    let s = Srf::of(&r.create[0]).unwrap();
    let corners = [s.at_st(0.0, 0.0), s.at_st(1.0, 1.0)];
    assert!(
        corners.iter().any(|c| dist(*c, [0.0; 3]) < 1e-6),
        "{corners:?}"
    );
    let r = run(
        PATCH,
        vec![curve("c", circle_pts([0.0; 3], 3.0, 0, 64), true)],
        json!({}),
        vec![Input::Enter],
    );
    // Rhino 5 Patch: a fitted surface trimmed to the closed curve.
    let Geo::Trimmed { surface, loops, .. } = &r.create[0] else { panic!("trimmed patch") };
    assert_eq!(loops.len(), 1);
    let s = Srf::of(surface).unwrap();
    assert!(norm(s.at_st(0.5, 0.5)) < 0.3, "{:?}", s.at_st(0.5, 0.5));
}

#[test]
fn revolve_is_an_exact_cylinder() {
    let prof = line("p", [5.0, 0.0, 0.0], [5.0, 0.0, 10.0]);
    let r = run(
        REVOLVE,
        vec![prof],
        json!({}),
        vec![pt([0.0; 3]), pt([0.0, 0.0, 1.0]), Input::Enter],
    );
    let s = Srf::of(&r.create[0]).unwrap();
    for i in 0..=40 {
        for j in 0..=4 {
            let p = s.at_st(i as f64 / 40.0, j as f64 / 4.0);
            assert!(
                ((p[0] * p[0] + p[1] * p[1]).sqrt() - 5.0).abs() < 1e-9,
                "{p:?}"
            );
        }
    }
    let r = run(
        EXTRUDE_ALL,
        vec![curve("c", circle_pts([0.0; 3], 2.0, 0, 64), true)],
        json!({"ea_distance": 3.0}),
        vec![Input::Enter],
    );
    let s = Srf::of(&r.create[0]).unwrap();
    assert!((s.at_st(1.0, 0.3)[2] - 3.0).abs() < 1e-9);
}

#[test]
fn planar_with_a_hole_and_plane() {
    let outer = curve("o", circle_pts([0.0; 3], 5.0, 0, 64), true);
    let hole = curve("h", circle_pts([0.0; 3], 2.0, 0, 32), true);
    let r = run(PLANAR, vec![outer, hole], json!({}), vec![Input::Enter]);
    assert_eq!(r.create.len(), 1);
    let Geo::Trimmed { loops, display, .. } = &r.create[0] else {
        panic!("a trimmed plane: {:?}", r.create[0]);
    };
    assert_eq!(loops.len(), 2, "outer and hole");
    if let Some(Geo::Mesh {
        vertices,
        triangles,
        ..
    }) = display.as_deref()
    {
        let area: f64 = triangles
            .iter()
            .map(|t| {
                norm(cross(
                    sub(vertices[t[1] as usize], vertices[t[0] as usize]),
                    sub(vertices[t[2] as usize], vertices[t[0] as usize]),
                )) / 2.0
            })
            .sum();
        let want = PI * 25.0 - PI * 4.0;
        assert!((area / want - 1.0).abs() < 0.02, "{area} vs {want}");
    }
    let r = run(
        PLANE,
        vec![],
        json!({}),
        vec![pt([0.0; 3]), pt([2.0, 3.0, 0.0])],
    );
    let s = Srf::of(&r.create[0]).unwrap();
    assert!(dist(s.at_st(1.0, 1.0), [2.0, 3.0, 0.0]) < 1e-12);
}

#[test]
fn offset_tween_rebuild_and_split() {
    let a = flat("a", 0.0, 4.0, 0.0, 4.0, 0.0);
    let r = run(
        OFFSET,
        vec![a.clone()],
        json!({"os_distance": 1.5}),
        vec![Input::Enter],
    );
    let s = Srf::of(&r.create[0]).unwrap();
    assert!((s.at_st(0.3, 0.7)[2].abs() - 1.5).abs() < 1e-6);
    let b = flat("b", 0.0, 4.0, 0.0, 4.0, 2.0);
    let r = run(
        TWEEN,
        vec![a.clone(), b],
        json!({"tw_count": 1}),
        vec![Input::Enter],
    );
    assert!((Srf::of(&r.create[0]).unwrap().at_st(0.5, 0.5)[2] - 1.0).abs() < 1e-6);
    let r = run(
        REBUILD,
        vec![a.clone()],
        json!({"sr_u_points": 6, "sr_v_points": 5}),
        vec![Input::Enter],
    );
    if let Geo::Surface { size, .. } = &r.replace[0].geo {
        assert_eq!(*size, [6, 5]);
    }
    let r = run(
        SPLIT_ISO,
        vec![a.clone()],
        json!({}),
        vec![pt([1.0, 2.0, 0.0])],
    );
    let first = Srf::of(&r.replace[0].geo).unwrap();
    let second = Srf::of(&r.create[0]).unwrap();
    assert!(
        (first.at_st(1.0, 0.5)[0] - 1.0).abs() < 0.01
            && (second.at_st(0.0, 0.5)[0] - 1.0).abs() < 0.01
    );
    let r = run(
        EXTEND,
        vec![a],
        json!({}),
        vec![num(2.0), pt([4.0, 2.0, 0.0])],
    );
    let ext = Srf::of(&r.replace[0].geo).unwrap();
    let xmax = (0..=10)
        .map(|k| ext.at_st(1.0, k as f64 / 10.0)[0])
        .fold(f64::MIN, f64::max)
        .max(
            (0..=10)
                .map(|k| ext.at_st(0.0, k as f64 / 10.0)[0])
                .fold(f64::MIN, f64::max),
        );
    assert!((xmax - 6.0).abs() < 1e-3, "{xmax}");
}

fn num(v: f64) -> Input {
    Input::Text { text: fmt(v) }
}

#[test]
fn blend_merge_and_match_edges() {
    let a = flat("a", 0.0, 4.0, 0.0, 4.0, 0.0);
    let b = flat("b", 6.0, 10.0, 0.0, 4.0, 1.0);
    let r = run(
        BLEND,
        vec![a.clone(), b.clone()],
        json!({}),
        vec![Input::Enter],
    );
    let s = Srf::of(&r.create[0]).unwrap();
    let ends = [s.at_st(0.0, 0.5), s.at_st(1.0, 0.5)];
    assert!(
        ends.iter().any(|p| (p[0] - 4.0).abs() < 1e-6)
            && ends.iter().any(|p| (p[0] - 6.0).abs() < 1e-6),
        "{ends:?}"
    );
    let c = flat("c", 4.0, 8.0, 0.0, 4.0, 0.0);
    let r = run(MERGE, vec![a.clone(), c], json!({}), vec![Input::Enter]);
    let m = Srf::of(&r.replace[0].geo).unwrap();
    let xs: Vec<f64> = [
        m.at_st(0.0, 0.0)[0],
        m.at_st(1.0, 0.0)[0],
        m.at_st(0.0, 1.0)[0],
        m.at_st(1.0, 1.0)[0],
    ]
    .to_vec();
    assert!(
        xs.iter().any(|x| x.abs() < 1e-6) && xs.iter().any(|x| (x - 8.0).abs() < 1e-6),
        "{xs:?}"
    );
    assert_eq!(r.delete.len(), 1);
    let r = run(
        MATCH,
        vec![a, b],
        json!({"sm_continuity": "position"}),
        vec![Input::Enter],
    );
    let m = Srf::of(&r.replace[0].geo).unwrap();
    let pts: Vec<Vec3> = (0..=4)
        .flat_map(|i| (0..=4).map(move |j| (i, j)))
        .map(|(i, j)| m.at_st(i as f64 / 4.0, j as f64 / 4.0))
        .collect();
    assert!(
        pts.iter()
            .any(|p| (p[0] - 6.0).abs() < 1e-3 && (p[2] - 1.0).abs() < 1e-3),
        "the matched edge reaches b"
    );
}

#[test]
fn fillet_and_chamfer_two_planes() {
    // A floor and a wall meeting along y at x = 0 (the corner region is x > 0, z > 0).
    let floor = Selected {
        id: "f".into(),
        kind: "mesh".into(),
        mesh: Some(
            om9_addon_geom::fit::grid_mesh(
                &(0..3)
                    .map(|i| {
                        (0..3)
                            .map(|j| [-5.0 + 7.5 * i as f64, -5.0 + 5.0 * j as f64, 0.0])
                            .collect()
                    })
                    .collect::<Vec<_>>(),
            )
            .into_sel(),
        ),
        ..Default::default()
    };
    let wall = Selected {
        id: "w".into(),
        kind: "mesh".into(),
        mesh: Some(
            om9_addon_geom::fit::grid_mesh(
                &(0..3)
                    .map(|i| {
                        (0..3)
                            .map(|j| [0.0, -5.0 + 5.0 * j as f64, -5.0 + 7.5 * i as f64])
                            .collect()
                    })
                    .collect::<Vec<_>>(),
            )
            .into_sel(),
        ),
        ..Default::default()
    };
    let fm = floor.mesh.clone().unwrap();
    let wm = wall.mesh.clone().unwrap();
    // Choose the sides so the ball sits in x > 0, z > 0.
    let (_, n1) = mesh_point(&fm, [1.0, 0.0, 0.0]).unwrap();
    let (_, n2) = mesh_point(&wm, [0.0, 0.0, 1.0]).unwrap();
    let flip = [n1[2] > 0.0, n2[0] > 0.0];
    let rows = fillet_rows(&fm, &wm, &|_| 1.0, flip, false, 16).unwrap();
    for r in &rows {
        for p in r {
            // On the circle of radius 1 around (1, y, 1).
            assert!(
                ((p[0] - 1.0).powi(2) + (p[2] - 1.0).powi(2)).sqrt() - 1.0 < 1e-6,
                "{p:?}"
            );
        }
        assert!(
            dist(r[0], [1.0, r[0][1], 0.0]) < 1e-6 || dist(r[0], [0.0, r[0][1], 1.0]) < 1e-6,
            "{:?}",
            r[0]
        );
    }
    let r = run(
        CHAMFER,
        vec![floor, wall],
        json!({"fs_radius": 1.0, "fs_flip_first": flip[0], "fs_flip_second": flip[1]}),
        vec![Input::Enter],
    );
    assert_eq!(r.create.len(), 1);
}

trait IntoSel {
    fn into_sel(self) -> SelectedMesh;
}

impl IntoSel for Mesh {
    fn into_sel(self) -> SelectedMesh {
        SelectedMesh {
            vertices: self.vertices,
            triangles: self.triangles,
        }
    }
}

#[test]
fn drape_unroll_and_cutting_plane() {
    let block = Selected {
        id: "b".into(),
        kind: "mesh".into(),
        mesh: Some(cuboid([-2.0, -2.0, 0.0], [2.0, 2.0, 3.0]).into_sel()),
        bbox: [[-2.0, -2.0, 0.0], [2.0, 2.0, 3.0]],
        ..Default::default()
    };
    let r = run(
        DRAPE,
        vec![block.clone()],
        json!({"dr_density": 12}),
        vec![pt([-1.0, -1.0, 0.0]), pt([1.0, 1.0, 0.0])],
    );
    let s = Srf::of(&r.create[0]).unwrap();
    assert!((s.at_st(0.5, 0.5)[2] - 3.0).abs() < 1e-6);
    // A half cylinder unrolls to a rectangle π r × length.
    let rows: Vec<Vec<Vec3>> = (0..2)
        .map(|i| {
            (0..=32)
                .map(|j| {
                    let a = PI * j as f64 / 32.0;
                    [2.0 * a.cos(), 2.0 * a.sin(), 5.0 * i as f64]
                })
                .collect()
        })
        .collect();
    let cyl = srf_sel("c", surface_through(&rows, [1, 3]).unwrap());
    let r = run(UNROLL, vec![cyl], json!({}), vec![Input::Enter]);
    if let Geo::Mesh { vertices, .. } = &r.create[0] {
        let xs = vertices.iter().map(|v| v[0]).fold(f64::MIN, f64::max)
            - vertices.iter().map(|v| v[0]).fold(f64::MAX, f64::min);
        let ys = vertices.iter().map(|v| v[1]).fold(f64::MIN, f64::max)
            - vertices.iter().map(|v| v[1]).fold(f64::MAX, f64::min);
        let (a, b) = (xs.max(ys), xs.min(ys));
        assert!(
            (a - 2.0 * PI).abs() < 0.05 && (b - 5.0).abs() < 0.05,
            "{xs} × {ys}"
        );
    }
    let r = run(
        CUTTING_PLANE,
        vec![block],
        json!({}),
        vec![pt([-5.0, 0.0, 0.0]), pt([5.0, 0.0, 0.0])],
    );
    let s = Srf::of(&r.create[0]).unwrap();
    assert!(s.at_st(0.5, 0.5)[1].abs() < 1e-9);
}

#[test]
fn soft_edit_seam_tangent_refit() {
    let a = flat("a", 0.0, 10.0, 0.0, 10.0, 0.0);
    let r = run(
        SOFT_EDIT,
        vec![a.clone()],
        json!({}),
        vec![num(0.3), pt([5.0, 5.0, 0.0]), pt([5.0, 5.0, 2.0])],
    );
    let s = Srf::of(&r.replace[0].geo).unwrap();
    assert!((s.at_st(0.5, 0.5)[2] - 2.0).abs() < 0.05 && s.at_st(0.0, 0.0)[2].abs() < 1e-6);
    let rows: Vec<Vec<Vec3>> = (0..=24)
        .map(|i| {
            let a = TAU * i as f64 / 24.0;
            (0..3)
                .map(|j| [3.0 * a.cos(), 3.0 * a.sin(), j as f64])
                .collect()
        })
        .collect();
    let tube = srf_sel("t", surface_through(&rows, [3, 2]).unwrap());
    let r = run(SEAM, vec![tube], json!({}), vec![pt([-3.0, 0.0, 1.0])]);
    let s = Srf::of(&r.replace[0].geo).unwrap();
    assert!(
        dist(s.at_st(0.0, 0.5), [-3.0, 0.0, 1.0]) < 0.05,
        "{:?}",
        s.at_st(0.0, 0.5)
    );
    let r = run(
        SET_TANGENT,
        vec![a.clone()],
        json!({}),
        vec![pt([10.0, 5.0, 0.0]), pt([9.0, 5.0, -1.0])],
    );
    assert_eq!(r.replace.len(), 1);
    let r = run(
        REFIT,
        vec![a],
        json!({"srf_tolerance": 0.001}),
        vec![Input::Enter],
    );
    assert_eq!(r.replace.len(), 1);
}

#[test]
fn every_command_starts() {
    for c in Surfaces::SESSION_COMMANDS {
        let r = Surfaces::step(&event(c, Input::Start, vec![], json!({}))).unwrap();
        assert!(!r.done && !r.prompt.is_empty(), "{c}");
    }
}

fn exact_circle(id: &str, c: Vec3, r: f64, x: Vec3, y: Vec3) -> Selected {
    let g = Geo::Circle { center: c, x, y, radius: r };
    let pts: Vec<Vec3> = (0..256).map(|k| om9_addon_geom::eval_geo(&g, k as f64 / 256.0).unwrap()).collect();
    Selected { exact: Some(g), ..curve(id, pts, true) }
}

fn loft_with(sel: Vec<Selected>, style: &str, closed: bool) -> Geo {
    run(LOFT, sel, json!({"loft_style": style, "loft_closed": closed}), vec![Input::Enter]).create.remove(0)
}

/// Rhino 5 Loft keeps its sections: two equal circles make an exact cylinder, and every
/// section of a normal, tight, uniform or straight loft lies exactly on the surface.
#[test]
fn loft_is_exact_through_its_sections_like_rhino5() {
    let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let t0 = std::time::Instant::now();
    let g = loft_with(vec![exact_circle("a", [0.0; 3], 5.0, x, y), exact_circle("b", [0.0, 0.0, 10.0], 5.0, x, y)], "normal", false);
    let s = Srf::of(&g).unwrap();
    for i in 0..=100 {
        for j in 0..=100 {
            let p = s.at_st(i as f64 / 100.0, j as f64 / 100.0);
            assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - 5.0).abs() < 1e-9, "{p:?}");
        }
    }
    // Three sections, the middle one wound the other way and started elsewhere: still on the
    // surface, no twist (the middle of the surface keeps a circle's radius 8 there).
    let secs = || vec![
        exact_circle("a", [0.0; 3], 5.0, x, y),
        exact_circle("b", [0.0, 0.0, 5.0], 8.0, [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        exact_circle("c", [0.0, 0.0, 10.0], 5.0, x, y),
    ];
    for style in ["normal", "tight", "uniform", "straight"] {
        let g = loft_with(secs(), style, false);
        let s = Srf::of(&g).unwrap();
        for (z, r) in [(0.0, 5.0), (5.0, 8.0), (10.0, 5.0)] {
            // The isocurve through that section: find u where z matches.
            let u = (0..=2000).map(|i| i as f64 / 2000.0).min_by(|a, b| (s.at_st(*a, 0.3)[2] - z).abs().total_cmp(&(s.at_st(*b, 0.3)[2] - z).abs())).unwrap();
            for j in 0..=40 {
                let p = s.at_st(u, j as f64 / 40.0);
                assert!((p[2] - z).abs() < 1e-6 && ((p[0] * p[0] + p[1] * p[1]).sqrt() - r).abs() < 1e-6, "{style} {p:?}");
            }
        }
        // No twist: a straight line of the surface keeps its angle (x/y sign) from bottom to top.
        let (a, b) = (s.at_st(0.0, 0.1), s.at_st(1.0, 0.1));
        assert!(a[0] * b[0] + a[1] * b[1] > 0.0, "{style} twisted {a:?} {b:?}");
    }
    // Loose: through the first and last only, inside the others' hull.
    let g = loft_with(secs(), "loose", false);
    let s = Srf::of(&g).unwrap();
    assert!((s.at_st(0.0, 0.2)[2]).abs() < 1e-9 && (s.at_st(1.0, 0.2)[2] - 10.0).abs() < 1e-9);
    // Closed loft of four sections: through all, and the same at both ends of u.
    let ring = vec![
        exact_circle("a", [10.0, 0.0, 0.0], 1.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        exact_circle("b", [0.0, 10.0, 0.0], 1.5, [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        exact_circle("c", [-10.0, 0.0, 0.0], 1.0, [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        exact_circle("d", [0.0, -10.0, 0.0], 1.5, [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    let g = loft_with(ring, "normal", true);
    let s = Srf::of(&g).unwrap();
    for j in 0..=20 {
        let v = j as f64 / 20.0;
        assert!(dist(s.at_st(0.0, v), s.at_st(1.0, v)) < 1e-9);
        let p = s.at_st(0.0, v);
        assert!((dist(p, [10.0, 0.0, 0.0]) - 1.0).abs() < 1e-9, "{p:?}");
    }
    // Mixed open sections (line, arc, cubic) are made compatible.
    let line = Selected { exact: Some(Geo::Line { start: [0.0; 3], end: [4.0, 0.0, 0.0] }), ..line("l", [0.0; 3], [4.0, 0.0, 0.0]) };
    let arcg = Geo::Arc { center: [2.0, 0.0, 3.0], x: [-1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], radius: 2.0, angle: PI };
    let arc_pts: Vec<Vec3> = (0..=64).map(|k| om9_addon_geom::eval_geo(&arcg, k as f64 / 64.0).unwrap()).collect();
    let arc = Selected { exact: Some(arcg), ..curve("a", arc_pts, false) };
    let g = loft_with(vec![line, arc], "normal", false);
    let s = Srf::of(&g).unwrap();
    for j in 0..=40 {
        let p = s.at_st(1.0, j as f64 / 40.0);
        assert!((dist(p, [2.0, 0.0, 3.0]) - 2.0).abs() < 1e-9, "{p:?}");
    }
    assert!(t0.elapsed().as_secs_f64() < 1.0, "{:?}", t0.elapsed());
}

/// Revolving an exact circle (Rhino 5: rational) gives an exact torus.
#[test]
fn revolve_of_a_circle_is_an_exact_torus() {
    let prof = exact_circle("p", [10.0, 0.0, 0.0], 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let r = run(REVOLVE, vec![prof], json!({}), vec![pt([0.0; 3]), pt([0.0, 0.0, 1.0]), Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    for i in 0..=60 {
        for j in 0..=60 {
            let p = s.at_st(i as f64 / 60.0, j as f64 / 60.0);
            let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert!((((rho - 10.0).powi(2) + p[2] * p[2]).sqrt() - 2.0).abs() < 1e-9, "{p:?}");
        }
    }
}

fn exact_sel(id: &str, g: Geo, closed: bool) -> Selected {
    let pts: Vec<Vec3> = (0..=128).map(|k| om9_addon_geom::eval_geo(&g, k as f64 / 128.0).unwrap()).collect();
    Selected { exact: Some(g), ..curve(id, pts, closed) }
}

/// Rhino 5 EdgeSrf: the surface's edges are exactly the picked curves (any order and
/// direction); two curves are opposite edges.
#[test]
fn edge_curves_keep_their_edges_exactly() {
    // A square of side 10 whose bottom edge is a half circle bulging down, others straight.
    let arc = Geo::Arc { center: [5.0, 0.0, 0.0], x: [-1.0, 0.0, 0.0], y: [0.0, -1.0, 0.0], radius: 5.0, angle: PI };
    let sel = vec![
        exact_sel("top", Geo::Line { start: [0.0, 10.0, 0.0], end: [10.0, 10.0, 0.0] }, false),
        exact_sel("arc", arc.clone(), false),
        exact_sel("left", Geo::Line { start: [0.0, 10.0, 0.0], end: [0.0, 0.0, 0.0] }, false),
        exact_sel("right", Geo::Line { start: [10.0, 0.0, 0.0], end: [10.0, 10.0, 0.0] }, false),
    ];
    let r = run(EDGE_CURVES, sel, json!({}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    // One edge is the arc: every point of it is 5 from (5,0,0), below y = 0.
    let edges = [(0, 0.0), (0, 1.0), (1, 0.0), (1, 1.0)];
    let mut found_arc = false;
    for (dir, at) in edges {
        let pts: Vec<Vec3> = (0..=50).map(|k| { let t = k as f64 / 50.0; if dir == 0 { s.at_st(at, t) } else { s.at_st(t, at) } }).collect();
        if pts.iter().all(|p| (dist(*p, [5.0, 0.0, 0.0]) - 5.0).abs() < 1e-9) {
            found_arc = true;
        }
    }
    assert!(found_arc, "the arc is an edge");
    // Flat inside (all on z = 0).
    for i in 0..=10 { for j in 0..=10 { assert!(s.at_st(i as f64 / 10.0, j as f64 / 10.0)[2].abs() < 1e-12); } }
    // Two curves: a ruled surface between them.
    let r = run(EDGE_CURVES, vec![
        exact_sel("a", Geo::Line { start: [0.0; 3], end: [4.0, 0.0, 0.0] }, false),
        exact_sel("b", Geo::Line { start: [4.0, 2.0, 1.0], end: [0.0, 2.0, 1.0] }, false),
    ], json!({}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    let mid = s.at_st(0.5, 0.5);
    assert!(dist(mid, [2.0, 1.0, 0.5]) < 1e-12, "{mid:?}");
}

fn opt(name: &str, value: Option<&str>) -> Input {
    Input::Option { name: name.into(), value: value.map(str::to_owned) }
}

/// Rhino 5 Revolve: curves, axis start, axis end, revolution angle (Enter = 360, typed, or a
/// click around the axis); FullCircle, StartAngle, Deformable/PointCount, DeleteInput.
#[test]
fn revolve_follows_rhino5_prompts_and_options() {
    let prof = || line("p", [5.0, 0.0, 0.0], [5.0, 0.0, 10.0]);
    let axis = || vec![pt([0.0; 3]), pt([0.0, 0.0, 1.0])];
    let angle_of = |g: &Geo| -> f64 {
        // The end of the turn (u = 1) at the profile's start: its angle around z.
        let s = Srf::of(g).unwrap();
        let p = s.at_st(1.0, 0.0);
        let a = p[1].atan2(p[0]).to_degrees();
        if a <= 1e-6 { a + 360.0 } else { a }
    };
    // The prompt asks for the angle after the axis, with 360 as default.
    let mut e = event(REVOLVE, Input::Start, vec![prof()], json!({}));
    let mut r = Surfaces::step(&e).unwrap();
    for i in axis() {
        e.state = r.state.clone();
        e.input = i;
        r = Surfaces::step(&e).unwrap();
    }
    assert!(!r.done && r.prompt.contains("Revolution angle") && r.prompt.contains("360"), "{}", r.prompt);
    // The pointer turns the preview: at (0, 5, 0) it shows a quarter turn.
    e.state = r.state.clone();
    e.input = Input::Hover { point: [0.0, 5.0, 0.0] };
    let h = Surfaces::step(&e).unwrap();
    assert!((angle_of(&h.preview[0]) - 90.0).abs() < 1e-6);
    let mut steps = axis();
    steps.push(text("90"));
    let r = run(REVOLVE, vec![prof()], json!({}), steps);
    assert!((angle_of(&r.create[0]) - 90.0).abs() < 1e-9);
    // A click at (0, 5, 0) around the z axis: 90 degrees.
    let mut steps = axis();
    steps.push(pt([0.0, 5.0, 0.0]));
    let r = run(REVOLVE, vec![prof()], json!({}), steps);
    assert!((angle_of(&r.create[0]) - 90.0).abs() < 1e-6);
    // FullCircle wins over a typed angle; StartAngle turns the start.
    let mut steps = axis();
    steps.insert(0, opt("FullCircle", Some("Yes")));
    steps.push(text("45"));
    let r = run(REVOLVE, vec![prof()], json!({}), steps);
    let s = Srf::of(&r.create[0]).unwrap();
    assert!(dist(s.at_st(0.0, 0.0), s.at_st(1.0, 0.0)) < 1e-9, "full turn");
    let mut steps = axis();
    steps.insert(0, opt("StartAngle", Some("30")));
    steps.push(text("60"));
    let r = run(REVOLVE, vec![prof()], json!({}), steps);
    let s = Srf::of(&r.create[0]).unwrap();
    let p = s.at_st(0.0, 0.0);
    assert!((p[1].atan2(p[0]).to_degrees() - 30.0).abs() < 1e-6, "{p:?}");
    assert!((angle_of(&r.create[0]) - 90.0).abs() < 1e-6);
    // Deformable: degree 3, not rational, PointCount points around; still near the cylinder.
    let mut steps = axis();
    steps.insert(0, opt("Deformable", Some("Yes")));
    steps.insert(1, opt("PointCount", Some("12")));
    steps.push(Input::Enter);
    let r = run(REVOLVE, vec![prof()], json!({}), steps);
    match &r.create[0] {
        Geo::Surface { degree, weights, size, .. } => {
            assert_eq!((degree[0], size[0]), (3, 12));
            assert!(weights.iter().all(|w| (*w - 1.0).abs() < 1e-12));
        }
        other => panic!("{other:?}"),
    }
    // DeleteInput removes the profile in the same step.
    let mut steps = axis();
    steps.insert(0, opt("DeleteInput", Some("Yes")));
    steps.push(Input::Enter);
    let r = run(REVOLVE, vec![prof()], json!({}), steps);
    assert_eq!(r.delete, vec!["p".to_string()]);
}

fn text(t: &str) -> Input {
    Input::Text { text: t.into() }
}

/// Rhino 5 Plane draws its rectangle like Rectangle (corner/length/width, 3Point, Vertical,
/// Center) and has Deformable (degrees and point counts) instead of Rounded.
#[test]
fn plane_follows_rhino5_rectangle_and_deformable() {
    let corners = |g: &Geo| {
        let s = Srf::of(g).unwrap();
        [s.at_st(0.0, 0.0), s.at_st(1.0, 0.0), s.at_st(1.0, 1.0), s.at_st(0.0, 1.0)]
    };
    let has = |c: &[Vec3; 4], p: Vec3| c.iter().any(|q| dist(*q, p) < 1e-9);
    // Typed length then width.
    let r = run(PLANE, vec![], json!({}), vec![pt([1.0, 1.0, 0.0]), text("4"), text("2")]);
    let c = corners(&r.create[0]);
    let size = (dist(c[0], c[1]), dist(c[1], c[2]));
    assert!(((size.0 - 4.0).abs() < 1e-9 && (size.1 - 2.0).abs() < 1e-9) || ((size.0 - 2.0).abs() < 1e-9 && (size.1 - 4.0).abs() < 1e-9), "{size:?}");
    // Center: corners symmetric about the center.
    let r = run(PLANE, vec![], json!({}), vec![opt("Center", None), pt([0.0; 3]), pt([3.0, 2.0, 0.0])]);
    let c = corners(&r.create[0]);
    assert!(has(&c, [-3.0, -2.0, 0.0]) && has(&c, [3.0, 2.0, 0.0]));
    // Vertical: stands on the edge, up from the CPlane.
    let r = run(PLANE, vec![], json!({}), vec![opt("Vertical", None), pt([0.0; 3]), pt([5.0, 0.0, 0.0]), text("3")]);
    let c = corners(&r.create[0]);
    assert!(c.iter().any(|p| (p[2].abs() - 3.0).abs() < 1e-9), "{c:?}");
    // 3Point: edge then width.
    let r = run(PLANE, vec![], json!({}), vec![opt("3Point", None), pt([0.0; 3]), pt([4.0, 0.0, 0.0]), pt([2.0, 2.0, 0.0])]);
    let c = corners(&r.create[0]);
    assert!(has(&c, [0.0, 2.0, 0.0]) && has(&c, [4.0, 2.0, 0.0]), "{c:?}");
    // Deformable: degree 3 × 2, 6 × 5 poles, still the same flat rectangle.
    let r = run(PLANE, vec![], json!({}), vec![
        opt("Deformable", Some("Yes")), opt("UDegree", Some("3")), opt("VDegree", Some("2")),
        opt("UPointCount", Some("6")), opt("VPointCount", Some("5")),
        pt([0.0; 3]), pt([6.0, 4.0, 0.0]),
    ]);
    match &r.create[0] {
        Geo::Surface { degree, size, .. } => assert_eq!((*degree, *size), ([3, 2], [6, 5])),
        other => panic!("{other:?}"),
    }
    let s = Srf::of(&r.create[0]).unwrap();
    for i in 0..=10 {
        for j in 0..=10 {
            let p = s.at_st(i as f64 / 10.0, j as f64 / 10.0);
            assert!(p[2].abs() < 1e-12 && p[0] > -1e-9 && p[0] < 6.0 + 1e-9 && p[1] > -1e-9 && p[1] < 4.0 + 1e-9);
        }
    }
    assert!(dist(s.at_st(0.5, 0.5), [3.0, 2.0, 0.0]) < 1e-9);
    // Rounded is not a Plane option; the prompt names the plane.
    let e = event(PLANE, opt("Rounded", Some("Yes")), vec![], json!({}));
    let r = Surfaces::step(&e).unwrap();
    assert!(r.error.is_some() && r.prompt.contains("plane") && !r.options.iter().any(|o| o.name == "Rounded"), "{}", r.prompt);
}

/// Rhino 5 PlanarSrf: a plane trimmed by the exact curves (a circle stays a circle), inner
/// curves are holes, and curves that only overlap make separate surfaces.
#[test]
fn planar_curves_are_exact_trims_like_rhino5() {
    let outer = exact_circle("o", [0.0; 3], 5.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let hole = exact_circle("h", [1.0, 0.0, 0.0], 2.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let r = run(PLANAR, vec![outer, hole], json!({}), vec![Input::Enter]);
    let Geo::Trimmed { surface, loops, .. } = &r.create[0] else { panic!() };
    // Each loop is the exact NURBS circle (every point on the circle).
    for (ring, (c, rad)) in loops.iter().zip([([0.0, 0.0, 0.0], 5.0), ([1.0, 0.0, 0.0], 2.0)]) {
        let nc = Nc::from_geo(&ring[0]).unwrap();
        for i in 0..=100 {
            assert!((dist(nc.eval(i as f64 / 100.0), c) - rad).abs() < 1e-12);
        }
    }
    // The sheet covers the outer loop in its plane.
    let s = Srf::of(surface).unwrap();
    assert!(s.at_st(0.0, 0.0)[0] < -5.0 && s.at_st(1.0, 1.0)[0] > 5.0 && s.at_st(0.5, 0.5)[2].abs() < 1e-12);
    // Two overlapping circles: two surfaces, no holes.
    let a = exact_circle("a", [0.0; 3], 3.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let b = exact_circle("b", [4.0, 0.0, 0.0], 3.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let r = run(PLANAR, vec![a, b], json!({}), vec![Input::Enter]);
    assert_eq!(r.create.len(), 2);
    assert!(r.create.iter().all(|g| matches!(g, Geo::Trimmed { loops, .. } if loops.len() == 1)));
}

/// Rhino 5 ExtrudeCrv (Extrude All): curves, then the distance (typed, Enter, or a click; the
/// preview follows the pointer); exact surfaces; BothSides; DeleteInput.
#[test]
fn extrude_all_follows_rhino5_and_is_exact() {
    let circ = || exact_circle("c", [0.0; 3], 4.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let on_cylinder = |g: &Geo, z0: f64, z1: f64| {
        let s = Srf::of(g).unwrap();
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for i in 0..=40 {
            for j in 0..=40 {
                let p = s.at_st(i as f64 / 40.0, j as f64 / 40.0);
                assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - 4.0).abs() < 1e-9, "{p:?}");
                lo = lo.min(p[2]);
                hi = hi.max(p[2]);
            }
        }
        assert!((lo - z0).abs() < 1e-9 && (hi - z1).abs() < 1e-9, "{lo} {hi}");
    };
    let r = run(EXTRUDE_ALL, vec![circ()], json!({}), vec![text("5")]);
    on_cylinder(&r.create[0], 0.0, 5.0);
    // Enter: the default (2); a click 3 above the curve: 3.
    let r = run(EXTRUDE_ALL, vec![circ()], json!({}), vec![Input::Enter]);
    on_cylinder(&r.create[0], 0.0, 2.0);
    let r = run(EXTRUDE_ALL, vec![circ()], json!({}), vec![pt([9.0, 1.0, 3.0])]);
    on_cylinder(&r.create[0], 0.0, 3.0);
    // BothSides and DeleteInput.
    let r = run(EXTRUDE_ALL, vec![circ()], json!({}), vec![opt("BothSides", Some("Yes")), opt("DeleteInput", Some("Yes")), text("1.5")]);
    on_cylinder(&r.create[0], -1.5, 1.5);
    assert_eq!(r.delete, vec!["c".to_string()]);
    // The preview follows the pointer's height.
    let mut e = event(EXTRUDE_ALL, Input::Start, vec![circ()], json!({}));
    let s0 = Surfaces::step(&e).unwrap();
    e.state = s0.state.clone();
    e.input = Input::Hover { point: [0.0, 9.0, 7.0] };
    let h = Surfaces::step(&e).unwrap();
    on_cylinder(&h.preview[0], 0.0, 7.0);
    // Zero is refused, the command stays open.
    let mut e = event(EXTRUDE_ALL, Input::Start, vec![circ()], json!({}));
    let s0 = Surfaces::step(&e).unwrap();
    e.state = s0.state.clone();
    e.input = text("0");
    let z = Surfaces::step(&e).unwrap();
    assert!(!z.done && z.error.is_some());
}

fn point_sel(id: &str, p: Vec3) -> Selected {
    Selected { kind: "point".into(), ..curve(id, vec![p], false) }
}

/// Rhino 5 Loft options: a point may end (or start) the loft, Natural keeps the seams,
/// Rebuild / Refit simplify the sections.
#[test]
fn loft_point_seams_and_simplify_like_rhino5() {
    let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    // Circle r5 at z 0 to the point (0, 0, 10), straight: an exact cone.
    let r = run(LOFT, vec![exact_circle("a", [0.0; 3], 5.0, x, y), point_sel("p", [0.0, 0.0, 10.0])], json!({"loft_style": "straight"}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    for i in 0..=20 {
        for j in 0..=40 {
            let p = s.at_st(i as f64 / 20.0, j as f64 / 40.0);
            let want = 5.0 * (1.0 - p[2] / 10.0);
            assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - want).abs() < 1e-9, "{p:?}");
        }
    }
    assert!(dist(s.at_st(1.0, 0.3), [0.0, 0.0, 10.0]) < 1e-12);
    // A point in the middle is refused.
    let e = event(LOFT, Input::Enter, vec![exact_circle("a", [0.0; 3], 5.0, x, y), point_sel("p", [0.0, 0.0, 5.0]), exact_circle("b", [0.0, 0.0, 10.0], 5.0, x, y)], json!({}));
    let r = Surfaces::step(&e).unwrap();
    assert!(!r.done && r.error.is_some());
    // Natural: the second circle keeps its own start (turned 90°), so the surface's v = 0
    // line ends where that circle starts.
    let b = exact_circle("b", [0.0, 0.0, 10.0], 5.0, y, [-1.0, 0.0, 0.0]);
    let r = run(LOFT, vec![exact_circle("a", [0.0; 3], 5.0, x, y), b.clone()], json!({"loft_seams": "natural"}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    assert!(dist(s.at_st(1.0, 0.0), [0.0, 5.0, 10.0]) < 1e-9);
    let r = run(LOFT, vec![exact_circle("a", [0.0; 3], 5.0, x, y), b], json!({}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    assert!(dist(s.at_st(1.0, 0.0), [5.0, 0.0, 10.0]) < 1e-9, "automatic aligns the seam: {:?} {:?}", s.at_st(1.0, 0.0), s.at_st(0.0, 0.0));
    // Rebuild with 12 points: non-rational sections, near the circles.
    let r = run(LOFT, vec![exact_circle("a", [0.0; 3], 5.0, x, y), exact_circle("b", [0.0, 0.0, 10.0], 5.0, x, y)], json!({"loft_simplify": "rebuild", "loft_rebuild_points": 12}), vec![Input::Enter]);
    match &r.create[0] {
        Geo::Surface { weights, size, .. } => {
            assert!(weights.iter().all(|w| (*w - 1.0).abs() < 1e-12));
            assert!((12..=15).contains(&size[1]), "{size:?}");
        }
        other => panic!("{other:?}"),
    }
    // Refit within 0.001: every point of the surface within that of the cylinder.
    let r = run(LOFT, vec![exact_circle("a", [0.0; 3], 5.0, x, y), exact_circle("b", [0.0, 0.0, 10.0], 5.0, x, y)], json!({"loft_simplify": "refit", "loft_refit_tolerance": 0.001}), vec![Input::Enter]);
    let s = Srf::of(&r.create[0]).unwrap();
    for j in 0..=200 {
        let p = s.at_st(0.5, j as f64 / 200.0);
        assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - 5.0).abs() < 1e-3, "{p:?}");
    }
}

/// Feed one input with a given selection, carrying the state.
fn feed(command: &str, state: &Value, input: Input, selection: Vec<Selected>, settings: Value) -> Reply {
    let mut e = event(command, input, selection, settings);
    e.state = state.clone();
    Surfaces::step(&e).unwrap()
}

/// Rhino 5 Sweep1: rail, then cross sections in order (Point option), Enter, then the
/// options (Style, Closed, GlobalBlend, Simplify) with a preview, Enter creates.
#[test]
fn sweep1_follows_rhino5_prompts_and_options() {
    let rail = || exact_sel("rail", Geo::Line { start: [0.0; 3], end: [0.0, 0.0, 10.0] }, false);
    let ring = || exact_circle("c", [0.0; 3], 2.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let r = feed(SWEEP1, &Value::Null, Input::Start, vec![], json!({}));
    assert_eq!(r.prompt, "Select rail");
    let r = feed(SWEEP1, &r.state, Input::Selection, vec![rail()], json!({}));
    assert!(r.prompt.starts_with("Select cross section curves"), "{}", r.prompt);
    assert!(r.options.iter().any(|o| o.name == "Point"));
    let r = feed(SWEEP1, &r.state, Input::Selection, vec![rail(), ring()], json!({}));
    assert!(r.prompt.contains("1 selected"), "{}", r.prompt);
    // Point option: the sweep ends at the apex.
    let r = feed(SWEEP1, &r.state, opt("Point", None), vec![rail(), ring()], json!({}));
    assert_eq!(r.want, Want::Point);
    let r = feed(SWEEP1, &r.state, pt([0.0, 0.0, 10.0]), vec![rail(), ring()], json!({}));
    // Enter ends the selection: options and a preview, nothing created yet.
    let r = feed(SWEEP1, &r.state, Input::Enter, vec![rail(), ring()], json!({}));
    assert!(!r.done && r.error.is_none(), "{:?} {}", r.error, r.prompt);
    assert!(!r.preview.is_empty());
    for name in ["Style", "GlobalBlend", "Simplify", "RebuildPoints", "RefitTolerance"] {
        assert!(r.options.iter().any(|o| o.name == name), "{name}");
    }
    // Closed is only offered for a closed rail with two or more sections.
    assert!(!r.options.iter().any(|o| o.name == "Closed"));
    // Global shape blending: linear from the circle to the point, a cone.
    let r = feed(SWEEP1, &r.state, opt("GlobalBlend", None), vec![rail(), ring()], json!({}));
    let r = feed(SWEEP1, &r.state, Input::Enter, vec![rail(), ring()], json!({}));
    assert!(r.done, "{}", r.prompt);
    let s = Srf::of(&r.create[0]).unwrap();
    for i in 0..=20 {
        for j in 0..=20 {
            let p = s.at_st(i as f64 / 20.0, j as f64 / 20.0);
            let want = 2.0 * (1.0 - p[2] / 10.0);
            assert!((p[0].hypot(p[1]) - want).abs() < 1e-3, "{p:?}");
        }
    }
}

/// A ring: one profile around a closed rail is an exact surface of revolution; Roadlike Top
/// keeps it too (the rail lies flat).
#[test]
fn sweep1_ring_is_exact_like_rhino5() {
    let rail = exact_circle("rail", [0.0; 3], 10.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let prof = exact_circle("p", [10.0, 0.0, 0.0], 1.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    for style in ["freeform", "roadlike_top"] {
        let r = run(SWEEP1, vec![rail.clone(), prof.clone()], json!({"sw1_style": style}), vec![Input::Enter]);
        assert!(r.messages.iter().any(|m| m.contains("exact")), "{:?}", r.messages);
        let s = Srf::of(&r.create[0]).unwrap();
        for i in 0..=40 {
            for j in 0..=40 {
                let p = s.at_st(i as f64 / 40.0, j as f64 / 40.0);
                let d = (p[0].hypot(p[1]) - 10.0).hypot(p[2]);
                assert!((d - 1.0).abs() < 1e-9, "{style}: {d}");
            }
        }
    }
}

/// Rhino 5 Sweep2: first rail, second rail, cross sections, Enter, options (MaintainHeight,
/// Simplify …), Enter. The surface's edges run on the rails.
#[test]
fn sweep2_follows_rhino5_prompts_and_runs_on_the_rails() {
    let ra = || exact_sel("a", Geo::Line { start: [0.0; 3], end: [0.0, 10.0, 0.0] }, false);
    let rb = || exact_sel("b", Geo::Line { start: [4.0, 0.0, 0.0], end: [8.0, 10.0, 0.0] }, false);
    let arch = || exact_sel("s", Geo::Arc { center: [2.0, 0.0, 0.0], x: [-1.0, 0.0, 0.0], y: [0.0, 0.0, 1.0], radius: 2.0, angle: PI }, false);
    let r = feed(SWEEP2, &Value::Null, Input::Start, vec![], json!({}));
    assert_eq!(r.prompt, "Select first rail");
    let r = feed(SWEEP2, &r.state, Input::Selection, vec![ra()], json!({}));
    assert_eq!(r.prompt, "Select second rail");
    let r = feed(SWEEP2, &r.state, Input::Selection, vec![ra(), rb()], json!({}));
    assert!(r.prompt.starts_with("Select cross section curves"), "{}", r.prompt);
    let r = feed(SWEEP2, &r.state, Input::Selection, vec![ra(), rb(), arch()], json!({}));
    let r = feed(SWEEP2, &r.state, Input::Enter, vec![ra(), rb(), arch()], json!({}));
    assert!(!r.done && !r.preview.is_empty(), "{:?}", r.error);
    assert!(r.options.iter().any(|o| o.name == "MaintainHeight"));
    assert!(!r.options.iter().any(|o| o.name == "Style"));
    let r = feed(SWEEP2, &r.state, opt("MaintainHeight", None), vec![ra(), rb(), arch()], json!({}));
    let r = feed(SWEEP2, &r.state, Input::Enter, vec![ra(), rb(), arch()], json!({}));
    assert!(r.done, "{}", r.prompt);
    let s = Srf::of(&r.create[0]).unwrap();
    for i in 0..=20 {
        let u = i as f64 / 20.0;
        let (e0, e1, top) = (s.at_st(u, 0.0), s.at_st(u, 1.0), s.at_st(u, 0.5));
        assert!(e0[0].abs() < 1e-6, "edge on rail A {e0:?}");
        assert!((e1[0] - (4.0 + 0.4 * e1[1])).abs() < 2e-3, "edge on rail B {e1:?}");
        assert!((top[2] - 2.0).abs() < 2e-3, "maintain height: {top:?}");
    }
    // Preselected (Matrix way): the two longest curves are the rails.
    let r = run(SWEEP2, vec![ra(), rb(), arch()], json!({}), vec![Input::Enter]);
    assert!(r.done);
}

/// Rhino 5 RailRevolve: profile, rail, axis start, axis end (ScaleHeight); exact.
#[test]
fn rail_revolve_follows_rhino5_and_is_exact() {
    let prof = || exact_sel("p", Geo::Arc { center: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 0.0, 1.0], radius: 10.0, angle: PI / 2.0 }, false);
    let rail = || exact_circle("r", [0.0; 3], 10.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let r = feed(RAIL_REVOLVE, &Value::Null, Input::Start, vec![], json!({}));
    assert_eq!(r.prompt, "Select profile curve");
    assert!(r.options.iter().any(|o| o.name == "ScaleHeight"));
    let r = feed(RAIL_REVOLVE, &r.state, Input::Selection, vec![prof()], json!({}));
    assert_eq!(r.prompt, "Select rail curve");
    let r = feed(RAIL_REVOLVE, &r.state, Input::Selection, vec![prof(), rail()], json!({}));
    assert!(r.prompt.starts_with("Start of RailRevolve axis"), "{}", r.prompt);
    let r = feed(RAIL_REVOLVE, &r.state, pt([0.0; 3]), vec![prof(), rail()], json!({}));
    assert!(r.prompt.starts_with("End of RailRevolve axis"), "{}", r.prompt);
    let r = feed(RAIL_REVOLVE, &r.state, pt([0.0, 0.0, 5.0]), vec![prof(), rail()], json!({}));
    assert!(r.done, "{} {:?} {:?}", r.prompt, r.error, r.state);
    let s = Srf::of(&r.create[0]).unwrap();
    for i in 0..=30 {
        for j in 0..=10 {
            let p = s.at_st(i as f64 / 30.0, j as f64 / 10.0);
            assert!(((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 10.0).abs() < 1e-9, "hemisphere {p:?}");
        }
    }
}

/// Rhino 5 NetworkSrf: curves of both directions in any order; the surface holds them all.
#[test]
fn curve_network_runs_through_every_curve() {
    let h = |x: f64, y: f64| 1.5 * (x * 0.4).sin() * (y * 0.3).cos();
    let mut sel = Vec::new();
    let mut k = 0;
    let mut add = |pts: Vec<Vec3>| {
        k += 1;
        let g = om9_addon_geom::interpolate(&pts, 3, false).unwrap();
        sel.push(exact_sel(&format!("c{k}"), g, false));
    };
    for x in [0.0, 4.0, 9.0] {
        add((0..=8).map(|i| { let y = i as f64; [x, y, h(x, y)] }).collect());
    }
    for y in [0.0, 3.0, 8.0] {
        add((0..=9).map(|i| { let x = i as f64; [x, y, h(x, y)] }).collect());
    }
    let r = run(NETWORK, sel.clone(), json!({}), vec![Input::Enter]);
    assert!(r.done, "{:?}", r.error);
    assert!(r.messages.iter().any(|m| m.contains("3 × 3")), "{:?}", r.messages);
    let s = Srf::of(&r.create[0]).unwrap();
    for c in &sel {
        for p in c.points.iter().step_by(8) {
            let mut best = (f64::MAX, 0.0, 0.0);
            for a in 0..=90 {
                for b in 0..=90 {
                    let (u, v) = (a as f64 / 90.0, b as f64 / 90.0);
                    let d = dist(*p, s.at_st(u, v));
                    if d < best.0 {
                        best = (d, u, v);
                    }
                }
            }
            let mut d = best.0;
            for a in -40..=40 {
                for b in -40..=40 {
                    let (u, v) = ((best.1 + a as f64 / 3600.0).clamp(0.0, 1.0), (best.2 + b as f64 / 3600.0).clamp(0.0, 1.0));
                    d = d.min(dist(*p, s.at_st(u, v)));
                }
            }
            assert!(d < 0.005, "{d}");
        }
    }
}

/// Rhino 5 Patch: a fit through boundary and interior curves, trimmed to the closed boundary.
#[test]
fn patch_fits_and_trims_like_rhino5() {
    let h = |x: f64, y: f64| 0.4 * (x / 3.0).sin() * (y / 4.0).cos();
    let mut sel = Vec::new();
    let mut add = |id: &str, f: &dyn Fn(f64) -> (f64, f64)| {
        let pts: Vec<Vec3> = (0..=12).map(|i| { let (x, y) = f(i as f64 / 12.0); [x, y, h(x, y)] }).collect();
        sel.push(exact_sel(id, om9_addon_geom::interpolate(&pts, 3, false).unwrap(), false));
    };
    add("b", &|t| (10.0 * t, 0.0));
    add("r", &|t| (10.0, 8.0 * t));
    add("t", &|t| (10.0 * (1.0 - t), 8.0));
    add("l", &|t| (0.0, 8.0 * (1.0 - t)));
    add("mid", &|t| (2.0 + 6.0 * t, 4.0));
    let r = run(PATCH, sel.clone(), json!({}), vec![Input::Enter]);
    assert!(r.done, "{:?}", r.error);
    let Geo::Trimmed { surface, loops, display } = &r.create[0] else { panic!("trimmed patch expected") };
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].len(), 4, "the four boundary curves");
    assert!(display.is_some());
    let s = Srf::of(surface).unwrap();
    // The interior curve lies on the patch.
    for p in sel[4].points.iter().step_by(6) {
        let mut best = f64::MAX;
        for a in 0..=100 {
            for b in 0..=100 {
                best = best.min(dist(*p, s.at_st(a as f64 / 100.0, b as f64 / 100.0)));
            }
        }
        assert!(best < 0.06, "{best}");
    }
    // Without automatic trim: the plain surface.
    let r = run(PATCH, sel, json!({"patch_auto_trim": false}), vec![Input::Enter]);
    assert!(matches!(r.create[0], Geo::Surface { .. }));
}

/// Rhino 5 OffsetSrf: select, Enter, Offset distance <d> (FlipAll Loose Tolerance BothSides
/// DeleteInput); a plane offsets exactly.
#[test]
fn offset_srf_follows_rhino5() {
    let a = flat("a", 0.0, 4.0, 0.0, 4.0, 0.0);
    let r = feed(OFFSET, &Value::Null, Input::Start, vec![a.clone()], json!({}));
    assert!(r.prompt.starts_with("Offset distance"), "{}", r.prompt);
    for name in ["FlipAll", "Loose", "Tolerance", "BothSides", "DeleteInput"] {
        assert!(r.options.iter().any(|o| o.name == name), "{name}");
    }
    assert!(!r.preview.is_empty(), "preview of the default distance");
    let r = feed(OFFSET, &r.state, opt("BothSides", None), vec![a.clone()], json!({}));
    let r = feed(OFFSET, &r.state, opt("DeleteInput", None), vec![a.clone()], json!({}));
    let r = feed(OFFSET, &r.state, text("0.6"), vec![a.clone()], json!({}));
    assert!(r.done, "{}", r.prompt);
    assert_eq!(r.create.len(), 2);
    assert_eq!(r.delete, vec!["a".to_string()]);
    let zs: Vec<f64> = r.create.iter().map(|g| Srf::of(g).unwrap().at_st(0.3, 0.6)[2]).collect();
    assert!(zs.iter().any(|z| (z - 0.6).abs() < 1e-12) && zs.iter().any(|z| (z + 0.6).abs() < 1e-12), "{zs:?}");
    assert!(r.messages.iter().any(|m| m.contains("exact")), "{:?}", r.messages);
}
