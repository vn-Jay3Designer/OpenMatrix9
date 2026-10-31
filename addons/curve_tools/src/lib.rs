// SPDX-License-Identifier: LGPL-2.1-or-later
//! The rest of the Matrix-style **Curve** menu for OM9 (the drawing commands are in `curves`):
//! rebuild and refit, blends (quick, adjustable, arc), curves from two views, project,
//! pullback, intersections, offsets (planar, on surface, normal to surface), isocurves,
//! fillets and chamfers, UV curves, through points, extend, mark start/end, seam, continue,
//! divide, curve boolean, tween, duplicate edge/border, cross sections, sketches, curves on
//! surfaces, curve to lines, section, match, silhouette, wireframe and soft edit.
use om9_addon_abi::session::{
    Event, Geo, Input, Interactive, Plane, Replacement, Reply, Selected, SelectedMesh, Vec3, Want,
    add, cross, dist, dot, fmt, norm, scale, sub, unit,
};
use om9_addon_geom::cmd::{Collected, Flow, Show, load, save};
use om9_addon_geom::nurbs::{self, Srf};
use om9_addon_geom::path::{Path, Target};
use om9_addon_geom::{Piece, composite_quadratic, interpolate, mesh, section, shapes};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const REBUILD: &str = "OthersCurveRebuild";
pub const REFIT: &str = "CurveCurveEditToolsRefitToTolerance";
pub const BLEND: &str = "CurveBlendCurvesBlendCurves";
pub const BLEND_ADJ: &str = "CurveBlendCurvesBlendCrv";
pub const CRV2VIEW: &str = "CurveCurveFrom2Views";
pub const CRV2VIEW_HISTORY: &str = "BuilderCrv2ViewHistory";
pub const PULLBACK: &str = "CurveCurveFromObjectsPullback";
pub const PROJECT: &str = "CurveCurveFromObjectsProject";
pub const INTERSECTION: &str = "CurveCurveFromObjectsIntersection";
pub const INTERSECT_TWO: &str = "CurveCurveFromObjectsIntersectTwoSets";
pub const OFFSET: &str = "CurveOffsetCurve";
pub const OFFSET_ON_SRF: &str = "CurveOffsetOffsetCrvOnSrf";
pub const OFFSET_NORMAL: &str = "CurveOffsetOffsetCrvNormalToSurface";
pub const ISOCURVE: &str = "CurveCurveFromObjectsExtractIsocurve";
pub const ANGLE_ISO: &str = "CurveGVExtractIsocurve";
pub const FILLET: &str = "CurveFilletCurves";
pub const FILLET_CORNERS: &str = "CurveFilletCorners";
pub const CHAMFER: &str = "CurveChamferCurves";
pub const UV_CREATE: &str = "CurveCurveFromObjectsCreateUVCurves";
pub const UV_APPLY: &str = "CurveCurveFromObjectsApplyUVCurves";
pub const THROUGH_POINTS: &str = "CurveFreeFormThroughPoints";
pub const EXTEND: &str = "CurveExtendCurveExtendCurve";
pub const MARK_START: &str = "CurvePointObjectMarkCurveStart";
pub const MARK_END: &str = "CurvePointObjectMarkCurveEnd";
pub const SEAM: &str = "CurveCurveEditToolsAdjustClosedCurveSeam";
pub const CONTINUE_INTERP: &str = "CurveFreeFormContinueInterpCurve";
pub const CONTINUE_CURVE: &str = "CurveFreeFormContinueCurve";
pub const DIVIDE: &str = "OthersCurveDivideCurve";
pub const CURVE_BOOLEAN: &str = "CurveCurveEditToolsCurveBoolean";
pub const TWEEN: &str = "CurveTweenCurves";
pub const DUP_EDGE: &str = "CurveCurveFromObjectsDuplicateEdge";
pub const DUP_BORDER: &str = "CurveCurveFromObjectsDuplicateBorder";
pub const CROSS_SECTION: &str = "CurveCrossSectionProfiles";
pub const SKETCH: &str = "CurveFreeFormSketch";
pub const SKETCH_SRF: &str = "CurveFreeFormSketchonSurface";
pub const SKETCH_MESH: &str = "CurveFreeFormSketchonPolygonMesh";
pub const POLY_ON_SRF: &str = "BuilderPolylineOnSurface";
pub const INTERP_ON_SRF: &str = "CurveFreeFormInterpolateonSurface";
pub const TO_LINES: &str = "CurveConvertCurveToLines";
pub const SECTION: &str = "CurveCurveFromObjectsSection";
pub const MATCH: &str = "CurveCurveEditToolsMatch";
pub const SILHOUETTE: &str = "CurveCurveFromObjectsSilhouette";
pub const WIREFRAME: &str = "CurveCurveFromObjectsExtractWireframe";
pub const SOFT_EDIT: &str = "CurveCurveEditToolsSoftEdit";
pub const ARC_BLEND: &str = "CurveBlendCurvesArcBlend";

mod blend;
mod edit;
mod extend;
mod fillet;
mod offset;
mod regions;
mod sections;

pub struct CurveTools;

impl Interactive for CurveTools {
    const SESSION_COMMANDS: &'static [&'static str] = &[
        REBUILD,
        REFIT,
        BLEND,
        BLEND_ADJ,
        CRV2VIEW,
        CRV2VIEW_HISTORY,
        PULLBACK,
        PROJECT,
        INTERSECTION,
        INTERSECT_TWO,
        OFFSET,
        OFFSET_ON_SRF,
        OFFSET_NORMAL,
        ISOCURVE,
        ANGLE_ISO,
        FILLET,
        FILLET_CORNERS,
        CHAMFER,
        UV_CREATE,
        UV_APPLY,
        THROUGH_POINTS,
        EXTEND,
        MARK_START,
        MARK_END,
        SEAM,
        CONTINUE_INTERP,
        CONTINUE_CURVE,
        DIVIDE,
        CURVE_BOOLEAN,
        TWEEN,
        DUP_EDGE,
        DUP_BORDER,
        CROSS_SECTION,
        SKETCH,
        SKETCH_SRF,
        SKETCH_MESH,
        POLY_ON_SRF,
        INTERP_ON_SRF,
        TO_LINES,
        SECTION,
        MATCH,
        SILHOUETTE,
        WIREFRAME,
        SOFT_EDIT,
        ARC_BLEND,
    ];
    fn step(e: &Event) -> Result<Reply, String> {
        Ok(match e.command.as_str() {
            REBUILD => edit::rebuild(e),
            REFIT => edit::fit_curve(e),
            BLEND => blend::blend(e, false),
            BLEND_ADJ => blend::blend(e, true),
            ARC_BLEND => blend::arc_blend(e),
            CRV2VIEW | CRV2VIEW_HISTORY => from_two_views(e),
            PULLBACK | PROJECT => project(e),
            INTERSECTION => intersection(e),
            INTERSECT_TWO => intersect_two_sets(e),
            OFFSET => offset::offset(e),
            OFFSET_ON_SRF | OFFSET_NORMAL => offset_surface(e),
            ISOCURVE | ANGLE_ISO => isocurve(e),
            FILLET | CHAMFER => fillet::fillet(e, e.command == CHAMFER),
            FILLET_CORNERS => fillet::fillet_corners(e),
            UV_CREATE => uv_create(e),
            UV_APPLY => uv_apply(e),
            THROUGH_POINTS => through_points(e),
            EXTEND => extend::extend(e),
            MARK_START | MARK_END => mark(e),
            SEAM => seam(e),
            CONTINUE_INTERP | CONTINUE_CURVE => continue_curve(e),
            DIVIDE => edit::divide(e),
            CURVE_BOOLEAN => regions::curve_boolean(e),
            TWEEN => edit::tween(e),
            DUP_EDGE | DUP_BORDER | SILHOUETTE | WIREFRAME => from_objects(e),
            CROSS_SECTION => sections::csec(e),
            SKETCH | SKETCH_SRF | SKETCH_MESH | POLY_ON_SRF | INTERP_ON_SRF => draw_on(e),
            TO_LINES => edit::convert(e),
            SECTION => sections::section_cmd(e),
            MATCH => blend::match_curve(e),
            SOFT_EDIT => soft_edit(e),
            other => return Err(format!("unknown command {other}")),
        })
    }
}

// ---- helpers ---------------------------------------------------------------------------------

fn curves(e: &Event) -> Vec<&Selected> {
    e.selection
        .iter()
        .filter(|s| s.kind == "curve" && s.points.len() >= 2)
        .collect()
}

fn meshes(e: &Event) -> Vec<&Selected> {
    e.selection
        .iter()
        .filter(|s| s.kind != "curve" && s.mesh.as_ref().is_some_and(|m| !m.triangles.is_empty()))
        .collect()
}

fn surfaces(e: &Event) -> Vec<(&Selected, Srf<'_>)> {
    e.selection
        .iter()
        .filter_map(|s| s.exact.as_ref().and_then(Srf::of).map(|f| (s, f)))
        .collect()
}

/// Dense samples of a selected curve: the exact curve when known, else OM9's samples.
fn dense(s: &Selected, n: usize) -> Vec<Vec3> {
    match &s.exact {
        Some(g @ (Geo::Line { .. } | Geo::Circle { .. } | Geo::Arc { .. } | Geo::Nurbs { .. })) => {
            let mut p = nurbs::samples(g, n);
            if s.closed && p.len() > 2 && dist(p[0], *p.last().unwrap()) < 1e-9 {
                p.pop();
            }
            p
        }
        _ => s.points.clone(),
    }
}

/// A smooth curve through points (a polyline when there are only two).
fn smooth(points: &[Vec3], closed: bool) -> Option<Geo> {
    if points.len() == 2 && !closed {
        return Some(Geo::Line {
            start: points[0],
            end: points[1],
        });
    }
    interpolate(points, 3, closed)
}

fn polyline(points: Vec<Vec3>, closed: bool) -> Geo {
    Geo::Polyline { points, closed }
}

fn ask_objects(prompt: String) -> Reply {
    Reply::ask(prompt, json!({})).want(Want::Objects)
}

/// Selection-only command: prompt until Enter, then `run`.
fn on_enter(e: &Event, prompt: &str, run: impl FnOnce() -> Result<Reply, String>) -> Reply {
    if !matches!(e.input, Input::Enter) {
        return ask_objects(prompt.to_owned());
    }
    match run() {
        Ok(r) => r,
        Err(w) => ask_objects(prompt.to_owned()).refuse(w),
    }
}

fn finish_curves(label: &str, geo: Vec<Geo>) -> Result<Reply, String> {
    if geo.is_empty() {
        return Err(format!("{label}: nothing to create"));
    }
    let n = geo.len();
    Ok(Reply::finish(geo).say(format!("{label}: {n} curves")))
}

fn replace(label: &str, items: Vec<(String, Geo)>) -> Result<Reply, String> {
    if items.is_empty() {
        return Err(format!("{label}: select curves"));
    }
    let n = items.len();
    let mut r = Reply::finish(Vec::new()).say(format!("{label}: {n} curves changed"));
    r.replace = items
        .into_iter()
        .map(|(id, geo)| Replacement { id, geo })
        .collect();
    Ok(r)
}

/// Resample a polyline to `n` points evenly by arc length.
fn resample(points: &[Vec3], closed: bool, n: usize) -> Vec<Vec3> {
    let Some(p) = Path::new(points, closed) else {
        return points.to_vec();
    };
    let n = n.max(2);
    let len = p.length();
    if closed {
        (0..n).map(|i| p.at(len * i as f64 / n as f64)).collect()
    } else {
        (0..n)
            .map(|i| p.at(len * i as f64 / (n - 1) as f64))
            .collect()
    }
}

// ---- curves from views, projections ---------------------------------------------------------

fn from_two_views(e: &Event) -> Reply {
    on_enter(
        e,
        "Curve from 2 Views: select the top-view curve and the front-view curve, Enter",
        || {
            let cs = curves(e);
            if cs.len() != 2 {
                return Err("select exactly two curves".into());
            }
            // The flatter curve (least spread along the normal) is the top view.
            let spread = |s: &Selected| {
                let z: Vec<f64> = s.points.iter().map(|&p| e.plane.local(p)[2]).collect();
                z.iter().cloned().fold(f64::MIN, f64::max)
                    - z.iter().cloned().fold(f64::MAX, f64::min)
            };
            let (top, front) = if spread(cs[0]) <= spread(cs[1]) {
                (cs[0], cs[1])
            } else {
                (cs[1], cs[0])
            };
            let fr: Vec<Vec3> = dense(front, 400)
                .iter()
                .map(|&p| e.plane.local(p))
                .collect();
            let height_at = |x: f64| -> Option<f64> {
                fr.windows(2).find_map(|w| {
                    let (a, b) = (w[0], w[1]);
                    ((a[0] - x) * (b[0] - x) <= 0.0 && (b[0] - a[0]).abs() > 1e-12)
                        .then(|| a[2] + (b[2] - a[2]) * (x - a[0]) / (b[0] - a[0]))
                })
            };
            let pts: Vec<Vec3> = dense(top, 200)
                .iter()
                .filter_map(|&p| {
                    let l = e.plane.local(p);
                    height_at(l[0]).map(|z| e.plane.at(l[0], l[1], z))
                })
                .collect();
            if pts.len() < 2 {
                return Err("the two curves must overlap along the CPlane x axis".into());
            }
            finish_curves(
                "Curve from 2 Views",
                smooth(&pts, top.closed && pts.len() > 3)
                    .into_iter()
                    .collect(),
            )
        },
    )
}

fn project(e: &Event) -> Reply {
    let pull = e.command == PULLBACK;
    let label = if pull { "Pullback" } else { "Project" };
    on_enter(
        e,
        &format!("{label}: select curves and the surface, Enter"),
        || {
            let ms = meshes(e);
            let m = ms
                .first()
                .ok_or("select the surface")?
                .mesh
                .as_ref()
                .unwrap();
            let n = e.plane.normal();
            let target = Target::new(m, n);
            let mut out = Vec::new();
            for s in curves(e) {
                let pts: Vec<Vec3> = dense(s, 200)
                    .iter()
                    .filter_map(|&p| {
                        if pull {
                            section::closest_on_mesh(m, p)
                        } else {
                            target.as_ref()?.drop(add(p, scale(n, 1e6))).map(|h| h.0)
                        }
                    })
                    .collect();
                if pts.len() >= 2 {
                    out.extend(smooth(&pts, s.closed));
                }
            }
            finish_curves(label, out)
        },
    )
}

fn chains_to_geo(chains: Vec<(Vec<Vec3>, bool)>) -> Vec<Geo> {
    chains.into_iter().map(|(p, c)| polyline(p, c)).collect()
}

fn intersection(e: &Event) -> Reply {
    on_enter(
        e,
        "Intersection: select two or more surfaces/solids (and planes), Enter",
        || {
            let ms = meshes(e);
            if ms.len() < 2 {
                return Err("select at least two surfaces or solids".into());
            }
            let mut out = Vec::new();
            for i in 0..ms.len() {
                for j in i + 1..ms.len() {
                    out.extend(chains_to_geo(section::mesh_intersection(
                        ms[i].mesh.as_ref().unwrap(),
                        ms[j].mesh.as_ref().unwrap(),
                    )));
                }
            }
            finish_curves("Intersection", out)
        },
    )
}

#[derive(Default, Serialize, Deserialize)]
struct FirstSet {
    meshes: Vec<(String, SelectedMesh)>,
}

fn intersect_two_sets(e: &Event) -> Reply {
    let mut st: FirstSet = load(e);
    let ms = meshes(e);
    if st.meshes.is_empty() {
        if matches!(e.input, Input::Enter) && !ms.is_empty() {
            st.meshes = ms
                .iter()
                .map(|s| (s.id.clone(), s.mesh.clone().unwrap()))
                .collect();
            return Reply::ask(
                "Intersect Two Sets: now select the second set, Enter",
                save(&st),
            )
            .want(Want::Objects);
        }
        return Reply::ask("Intersect Two Sets: select the first set, Enter", save(&st))
            .want(Want::Objects);
    }
    let second: Vec<&Selected> = ms
        .into_iter()
        .filter(|s| !st.meshes.iter().any(|m| m.0 == s.id))
        .collect();
    if !matches!(e.input, Input::Enter) || second.is_empty() {
        return Reply::ask(
            format!(
                "Intersect Two Sets: {} objects in the second set, Enter",
                second.len()
            ),
            save(&st),
        )
        .want(Want::Objects);
    }
    let mut out = Vec::new();
    for (_, a) in &st.meshes {
        for b in &second {
            out.extend(chains_to_geo(section::mesh_intersection(
                a,
                b.mesh.as_ref().unwrap(),
            )));
        }
    }
    finish_curves("Intersect Two Sets", out)
        .unwrap_or_else(|w| Reply::ask("Intersect Two Sets: select the second set, Enter", save(&st)).want(Want::Objects).refuse(w))
}

// ---- offsets ---------------------------------------------------------------------------------

/// Offset a curve in the CPlane by `d` toward `side` (a point), keeping corners.
pub fn offset_points(
    points: &[Vec3],
    closed: bool,
    plane: &Plane,
    d: f64,
    side: Option<Vec3>,
) -> Vec<Vec3> {
    let flat: Vec<[f64; 2]> = points
        .iter()
        .map(|&p| {
            let l = plane.local(p);
            [l[0], l[1]]
        })
        .collect();
    let z: Vec<f64> = points.iter().map(|&p| plane.local(p)[2]).collect();
    let n = flat.len();
    let normal_at = |i: usize| -> [f64; 2] {
        let (a, b) = if closed {
            (flat[(i + n - 1) % n], flat[(i + 1) % n])
        } else {
            (flat[i.saturating_sub(1)], flat[(i + 1).min(n - 1)])
        };
        let t = [b[0] - a[0], b[1] - a[1]];
        let l = (t[0] * t[0] + t[1] * t[1]).sqrt().max(1e-15);
        [t[1] / l, -t[0] / l]
    };
    // Which side: the right-hand normal, flipped when the side point lies left.
    let mut sign = 1.0;
    if let Some(sp) = side {
        let l = plane.local(sp);
        let (i, _) = flat
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let da = (a.1[0] - l[0]).powi(2) + (a.1[1] - l[1]).powi(2);
                let db = (b.1[0] - l[0]).powi(2) + (b.1[1] - l[1]).powi(2);
                da.total_cmp(&db)
            })
            .unwrap();
        let nn = normal_at(i);
        if (l[0] - flat[i][0]) * nn[0] + (l[1] - flat[i][1]) * nn[1] < 0.0 {
            sign = -1.0;
        }
    }
    let off = if closed {
        let o = if shapes::area2(&flat) > 0.0 {
            mesh::offset_outline(&flat, d * sign)
        } else {
            mesh::offset_outline(&flat, -d * sign)
        };
        // offset_outline pushes along the outward normal of a counter-clockwise outline.
        let ccw = shapes::area2(&flat) > 0.0;
        let _ = ccw;
        o
    } else {
        (0..n)
            .map(|i| {
                let nn = normal_at(i);
                [flat[i][0] + nn[0] * d * sign, flat[i][1] + nn[1] * d * sign]
            })
            .collect()
    };
    off.iter()
        .zip(&z)
        .map(|(q, &h)| plane.at(q[0], q[1], h))
        .collect()
}

fn offset_surface(e: &Event) -> Reply {
    let normal = e.command == OFFSET_NORMAL;
    let label = if normal {
        "Offset Curve Normal to Surface"
    } else {
        "Offset Curve on Surface"
    };
    let mut flow = Flow::new(label).number("Offset distance", 1.0);
    if !normal {
        flow = flow.point("Side to offset to");
    }
    let n = e.plane.normal();
    let make = |c: &Collected| -> Result<Vec<Geo>, String> {
        let ms = meshes(e);
        let m = ms
            .first()
            .ok_or("select the curves and the surface")?
            .mesh
            .as_ref()
            .unwrap();
        let target = Target::new(m, n).ok_or("surface")?;
        let d = c.n(0).ok_or("distance")?;
        let mut out = Vec::new();
        for s in curves(e) {
            let pts = dense(s, 200);
            let moved: Vec<Vec3> = if normal {
                pts.iter()
                    .map(|&p| {
                        target
                            .drop(add(p, scale(n, 1e6)))
                            .map_or(p, |(h, nn)| add(h, scale(nn, d)))
                    })
                    .collect()
            } else {
                offset_points(&pts, s.closed, &e.plane, d, c.p(0))
                    .iter()
                    .map(|&p| target.drop(add(p, scale(n, 1e6))).map_or(p, |h| h.0))
                    .collect()
            };
            out.extend(smooth(&moved, s.closed));
        }
        Ok(out)
    };
    flow.step(
        e,
        &|c, _| Show {
            geo: make(c).unwrap_or_default(),
            ..Default::default()
        },
        &|c| finish_curves(label, make(c)?),
    )
}

// ---- isocurves -------------------------------------------------------------------------------

fn isocurve(e: &Event) -> Reply {
    let angle_mode = e.command == ANGLE_ISO;
    let label = if angle_mode {
        "Extract Angle Isocurve"
    } else {
        "Extract Isocurve"
    };
    let mut flow = Flow::new(label);
    if angle_mode {
        flow = flow.number("Angle of the section (degrees from the CPlane x axis)", 0.0);
    } else {
        flow = flow.choice("Direction", &["U", "V", "Both"], "U");
    }
    let flow = flow.point("Point on the surface for the isocurve");
    let make = |c: &Collected, p: Vec3| -> Vec<Geo> {
        let mut out = Vec::new();
        if !angle_mode {
            for (_, s) in surfaces(e) {
                let (u, v) = s.closest_st(p);
                match c.opt(&flow, "Direction") {
                    "V" => out.push(s.isocurve(1, v)),
                    "Both" => {
                        out.push(s.isocurve(0, u));
                        out.push(s.isocurve(1, v));
                    }
                    _ => out.push(s.isocurve(0, u)),
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
        // Meshes: a section through the point, perpendicular to the CPlane at the angle.
        let a = c.n(0).unwrap_or(0.0).to_radians();
        let along = add(scale(e.plane.x, a.cos()), scale(e.plane.y, a.sin()));
        let sec_n = unit(cross(along, e.plane.normal()));
        for s in meshes(e) {
            out.extend(chains_to_geo(section::section(
                s.mesh.as_ref().unwrap(),
                p,
                sec_n,
            )));
        }
        out
    };
    flow.step(
        e,
        &|c, h| Show {
            geo: h.map(|p| make(c, p)).unwrap_or_default(),
            ..Default::default()
        },
        &|c| finish_curves(label, make(c, c.p(0).unwrap())),
    )
}

// ---- fillets and chamfers --------------------------------------------------------------------

/// Corner geometry: from the corner `x` the two legs leave along `u` and `v`.
fn fillet_arc(x: Vec3, u: Vec3, v: Vec3, r: f64) -> Option<(Vec3, Vec3, Vec3)> {
    let (u, v) = (unit(u), unit(v));
    let half = dot(u, v).clamp(-1.0, 1.0).acos() / 2.0;
    if half < 1e-6 || (std::f64::consts::FRAC_PI_2 - half) < 1e-9 {
        return None;
    }
    let t = r / half.tan();
    let a = add(x, scale(u, t));
    let b = add(x, scale(v, t));
    let bis = unit(add(u, v));
    let center = add(x, scale(bis, r / half.sin()));
    Some((a, b, center))
}

/// A polyline with every corner rounded by `r` (exact lines and arcs).
pub fn round_corners(points: &[Vec3], closed: bool, r: f64) -> Option<Geo> {
    let n = points.len();
    if n < 3 {
        return None;
    }
    let mut pieces = Vec::new();
    let corner = |i: usize| -> Option<(Vec3, Vec3, Vec3)> {
        let (p, x, q) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let (u, v) = (sub(p, x), sub(q, x));
        let max_r = norm(u).min(norm(v)) / 2.0;
        let half = dot(unit(u), unit(v)).clamp(-1.0, 1.0).acos() / 2.0;
        let r_fit = r.min(max_r * half.tan());
        fillet_arc(x, u, v, r_fit)
    };
    let range: Vec<usize> = if closed {
        (0..n).collect()
    } else {
        (1..n - 1).collect()
    };
    let corners: Vec<Option<(Vec3, Vec3, Vec3)>> = range.iter().map(|&i| corner(i)).collect();
    let mut cursor = if closed {
        corners[0].map(|c| c.1).unwrap_or(points[0])
    } else {
        points[0]
    };
    let iter: Vec<usize> = if closed {
        (1..=n).collect()
    } else {
        (0..corners.len()).collect()
    };
    for k in iter {
        let (idx, cr) = if closed {
            (k % n, corners[k % n])
        } else {
            (range[k], corners[k])
        };
        match cr {
            Some((a, b, center)) => {
                if dist(cursor, a) > 1e-9 {
                    pieces.push(Piece::Line(cursor, a));
                }
                // Arcs up to 90°: split bigger turns in two.
                let mid_dir = unit(sub(add(scale(add(a, b), 0.5), [0.0; 3]), center));
                let rr = dist(a, center);
                let m = add(center, scale(mid_dir, rr));
                let ang = dot(unit(sub(a, center)), unit(sub(b, center)))
                    .clamp(-1.0, 1.0)
                    .acos();
                if ang > std::f64::consts::FRAC_PI_2 {
                    pieces.push(Piece::Arc {
                        center,
                        start: a,
                        end: m,
                    });
                    pieces.push(Piece::Arc {
                        center,
                        start: m,
                        end: b,
                    });
                } else {
                    pieces.push(Piece::Arc {
                        center,
                        start: a,
                        end: b,
                    });
                }
                cursor = b;
            }
            None => {
                let x = points[idx];
                if dist(cursor, x) > 1e-9 {
                    pieces.push(Piece::Line(cursor, x));
                }
                cursor = x;
            }
        }
    }
    if !closed && dist(cursor, points[n - 1]) > 1e-9 {
        pieces.push(Piece::Line(cursor, points[n - 1]));
    }
    composite_quadratic(&pieces)
}

// ---- UV curves -------------------------------------------------------------------------------

fn uv_size(s: &Srf) -> (f64, f64) {
    let len = |g: &Geo| {
        let p = nurbs::samples(g, 64);
        p.windows(2)
            .map(|w| dist(w[0], w[1]))
            .sum::<f64>()
            .max(1e-6)
    };
    (len(&s.isocurve(1, 0.5)), len(&s.isocurve(0, 0.5)))
}

fn uv_create(e: &Event) -> Reply {
    on_enter(
        e,
        "Create UV Curves: select a surface (and curves on it), Enter",
        || {
            let srfs = surfaces(e);
            let (_, s) = srfs.first().ok_or("select a NURBS surface")?;
            let (w, h) = uv_size(s);
            let o = e.plane.origin;
            let at = |u: f64, v: f64| {
                e.plane.at(
                    e.plane.local(o)[0] + u * w,
                    e.plane.local(o)[1] + v * h,
                    0.0,
                )
            };
            let mut out = vec![polyline(
                vec![at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)],
                true,
            )];
            for c in curves(e) {
                let uv: Vec<Vec3> = dense(c, 120)
                    .iter()
                    .map(|&p| {
                        let (u, v) = s.closest_st(p);
                        at(u, v)
                    })
                    .collect();
                out.extend(smooth(&uv, c.closed));
            }
            finish_curves("Create UV Curves", out)
        },
    )
}

fn uv_apply(e: &Event) -> Reply {
    on_enter(
        e,
        "Apply UV Curves: select the flat curves and the target surface, Enter",
        || {
            let srfs = surfaces(e);
            let (_, s) = srfs.first().ok_or("select a NURBS surface")?;
            let (w, h) = uv_size(s);
            let o = e.plane.local(e.plane.origin);
            let mut out = Vec::new();
            for c in curves(e) {
                let pts: Vec<Vec3> = dense(c, 200)
                    .iter()
                    .map(|&p| {
                        let l = e.plane.local(p);
                        s.at_st((l[0] - o[0]) / w, (l[1] - o[1]) / h)
                    })
                    .collect();
                out.extend(smooth(&pts, c.closed));
            }
            finish_curves("Apply UV Curves", out)
        },
    )
}

// ---- points and simple curve edits ---------------------------------------------------------

fn through_points(e: &Event) -> Reply {
    let flow = Flow::new("Curve Through Points").choice(
        "Type",
        &["Interpolate", "Polyline"],
        "Interpolate",
    );
    let make = |c: &Collected| -> Result<Geo, String> {
        let pts: Vec<Vec3> = e
            .selection
            .iter()
            .filter(|s| s.kind == "point")
            .map(|s| scale(add(s.bbox[0], s.bbox[1]), 0.5))
            .collect();
        if pts.len() < 2 {
            return Err("select two or more points".into());
        }
        // Order as a line: nearest neighbour from the outermost point.
        let centroid = scale(
            pts.iter().fold([0.0; 3], |a, &p| add(a, p)),
            1.0 / pts.len() as f64,
        );
        let mut rest = pts.clone();
        let start = (0..rest.len())
            .max_by(|&a, &b| dist(rest[a], centroid).total_cmp(&dist(rest[b], centroid)))
            .unwrap();
        let mut order = vec![rest.remove(start)];
        while !rest.is_empty() {
            let last = *order.last().unwrap();
            let i = (0..rest.len())
                .min_by(|&a, &b| dist(rest[a], last).total_cmp(&dist(rest[b], last)))
                .unwrap();
            order.push(rest.remove(i));
        }
        Ok(if c.opt(&flow, "Type") == "Polyline" {
            polyline(order, false)
        } else {
            smooth(&order, false).ok_or("points")?
        })
    };
    if matches!(e.input, Input::Enter)
        && let Ok(g) = make(&load(e))
    {
        return Reply::finish(vec![g]).say("Curve Through Points: done");
    }
    let mut r = flow.step(
        e,
        &|c, _| Show {
            geo: make(c).into_iter().collect(),
            ..Default::default()
        },
        &|c| finish_curves("Curve Through Points", vec![make(c)?]),
    );
    r.prompt = "Curve Through Points: select point objects, Enter".into();
    r
}

fn mark(e: &Event) -> Reply {
    let end = e.command == MARK_END;
    let label = if end {
        "Mark Curve End"
    } else {
        "Mark Curve Start"
    };
    on_enter(e, &format!("{label}: select curves, Enter"), || {
        let mut out = Vec::new();
        for s in curves(e) {
            let p = dense(s, 50);
            let (a, b) = if end {
                (p[p.len() - 1], p[p.len() - 2])
            } else {
                (p[0], p[1])
            };
            out.push(Geo::Point { point: a });
            // A short arrow showing the curve direction there.
            let d = unit(if end { sub(a, b) } else { sub(b, a) });
            out.push(Geo::Line {
                start: a,
                end: add(a, scale(d, 1.0)),
            });
        }
        if out.is_empty() {
            return Err("select curves".into());
        }
        Ok(Reply::finish(out).say(format!("{label}: marked")))
    })
}

fn seam(e: &Event) -> Reply {
    let flow = Flow::new("Adjust Closed Curve Seam").point("New seam point on the curve");
    flow.step(e, &|_, _| Show::default(), &|c| {
        let p = c.p(0).unwrap();
        let items = curves(e)
            .iter()
            .filter(|s| s.closed)
            .filter_map(|s| {
                let mut pts = match &s.exact {
                    Some(Geo::Nurbs {
                        degree: 1, points, ..
                    }) => {
                        let mut q = points.clone();
                        q.pop();
                        q
                    }
                    _ => resample(&dense(s, 300), true, 64),
                };
                let path = Path::new(&pts, true)?;
                let s_at = path.closest(p);
                let seam_pt = path.at(s_at);
                let i = (0..pts.len())
                    .min_by(|&a, &b| dist(pts[a], seam_pt).total_cmp(&dist(pts[b], seam_pt)))?;
                pts.rotate_left(i);
                if dist(pts[0], seam_pt) > 1e-9 {
                    pts.insert(0, seam_pt);
                }
                let g = if matches!(&s.exact, Some(Geo::Nurbs { degree: 1, .. })) {
                    polyline(pts, true)
                } else {
                    smooth(&pts, true)?
                };
                Some((s.id.clone(), g))
            })
            .collect();
        replace("Adjust Closed Curve Seam", items)
    })
}

#[derive(Default, Serialize, Deserialize)]
struct Clicks {
    points: Vec<Vec3>,
}

fn continue_curve(e: &Event) -> Reply {
    let interp = e.command == CONTINUE_INTERP;
    let label = if interp {
        "Continue Interp Curve"
    } else {
        "Continue Control Point Curve"
    };
    let mut st: Clicks = load(e);
    let cs = curves(e);
    let Some(s) = cs.first() else {
        return ask_objects(format!("{label}: select the curve to continue"));
    };
    match &e.input {
        Input::Point { point } => st.points.push(*point),
        Input::Undo => {
            st.points.pop();
        }
        Input::Enter if !st.points.is_empty() => {
            let g = if interp {
                let mut pts = resample(&dense(s, 300), false, 16);
                pts.extend(&st.points);
                smooth(&pts, false)
            } else {
                let (degree, mut poles) = match &s.exact {
                    Some(Geo::Nurbs { degree, points, .. }) => (*degree, points.clone()),
                    _ => (3, resample(&s.points, false, 8)),
                };
                poles.extend(&st.points);
                om9_addon_geom::control_curve(&poles, degree)
            };
            return match g {
                Some(g) => {
                    let mut r = Reply::finish(Vec::new())
                        .say(format!("{label}: {} points added", st.points.len()));
                    r.replace.push(Replacement {
                        id: s.id.clone(),
                        geo: g,
                    });
                    r
                }
                None => Reply::ask(label, save(&st)).refuse("curve"),
            };
        }
        _ => {}
    }
    let last = st
        .points
        .last()
        .copied()
        .unwrap_or_else(|| *dense(s, 50).last().unwrap());
    Reply::ask(
        format!(
            "{label}: next point ({} added), Enter to finish",
            st.points.len()
        ),
        save(&st),
    )
    .want(Want::Point)
    .rubber(Some(last))
}

/// Curves from meshes and surfaces: Duplicate Edge (sharp edges and borders), Duplicate
/// Border, Silhouette (seen along the CPlane normal), Extract Wireframe.
fn from_objects(e: &Event) -> Reply {
    let cmd = e.command.as_str();
    let label = match cmd {
        DUP_EDGE => "Duplicate Edge",
        DUP_BORDER => "Duplicate Border",
        SILHOUETTE => "Silhouette",
        _ => "Extract Wireframe",
    };
    on_enter(
        e,
        &format!("{label}: select surfaces, solids or meshes, Enter"),
        || {
            let mut out = Vec::new();
            for (_, s) in surfaces(e) {
                match cmd {
                    WIREFRAME => {
                        for k in 0..=4 {
                            let t = k as f64 / 4.0;
                            out.push(s.isocurve(0, t));
                            out.push(s.isocurve(1, t));
                        }
                    }
                    SILHOUETTE => {}
                    _ => {
                        for t in [0.0, 1.0] {
                            out.push(s.isocurve(0, t));
                            out.push(s.isocurve(1, t));
                        }
                    }
                }
            }
            let exact_ids: Vec<&str> = surfaces(e).iter().map(|(s, _)| s.id.as_str()).collect();
            for s in meshes(e) {
                if exact_ids.contains(&s.id.as_str()) && cmd != SILHOUETTE {
                    continue;
                }
                let m = s.mesh.as_ref().unwrap();
                let segs = match cmd {
                    DUP_EDGE => section::feature_edges(m, 30.0, false),
                    DUP_BORDER => section::feature_edges(m, 0.0, true),
                    SILHOUETTE => section::silhouette(m, e.plane.normal()),
                    _ => section::feature_edges(m, 10.0, false),
                };
                out.extend(chains_to_geo(section::chain(&segs, 1e-7)));
            }
            finish_curves(label, out)
        },
    )
}

/// Sketches and curves through clicked points, optionally on a surface.
fn draw_on(e: &Event) -> Reply {
    let cmd = e.command.as_str();
    let label = match cmd {
        SKETCH => "Sketch",
        SKETCH_SRF => "Sketch on Surface",
        SKETCH_MESH => "Sketch on Polygon Mesh",
        POLY_ON_SRF => "Polyline on Surface",
        _ => "Interpolate on Surface",
    };
    let on_surface = cmd != SKETCH;
    let mut st: Clicks = load(e);
    let n = e.plane.normal();
    let ms = meshes(e);
    let target = ms
        .first()
        .and_then(|s| Target::new(s.mesh.as_ref().unwrap(), n));
    if on_surface && target.is_none() {
        return ask_objects(format!("{label}: select the surface or mesh first"));
    }
    let drop = |p: Vec3| -> Vec3 {
        target
            .as_ref()
            .and_then(|t| t.drop(add(p, scale(n, 1e6))))
            .map_or(p, |h| h.0)
    };
    match &e.input {
        Input::Point { point } => st.points.push(drop(*point)),
        Input::Undo => {
            st.points.pop();
        }
        Input::Enter if st.points.len() >= 2 => {
            let pts = st.points.clone();
            let g = match cmd {
                POLY_ON_SRF => {
                    // Each segment follows the surface.
                    let mut dense_pts = Vec::new();
                    for w in pts.windows(2) {
                        for i in 0..16 {
                            dense_pts
                                .push(drop(add(w[0], scale(sub(w[1], w[0]), i as f64 / 16.0))));
                        }
                    }
                    dense_pts.push(*pts.last().unwrap());
                    Some(polyline(dense_pts, false))
                }
                SKETCH | SKETCH_SRF | SKETCH_MESH => {
                    // Smooth the stroke a little, then fit.
                    let mut s = pts.clone();
                    for _ in 0..2 {
                        let prev = s.clone();
                        for i in 1..s.len() - 1 {
                            s[i] = scale(
                                add(add(prev[i - 1], prev[i + 1]), scale(prev[i], 2.0)),
                                0.25,
                            );
                        }
                    }
                    let fitted = smooth(&s, false);
                    if on_surface {
                        fitted.map(|g| {
                            let p: Vec<Vec3> =
                                nurbs::samples(&g, 120).into_iter().map(drop).collect();
                            smooth(&p, false).unwrap_or(g)
                        })
                    } else {
                        fitted
                    }
                }
                _ => smooth(&pts, false).map(|g| {
                    let p: Vec<Vec3> = nurbs::samples(&g, 120).into_iter().map(drop).collect();
                    smooth(&p, false).unwrap_or(g)
                }),
            };
            return match g {
                Some(g) => Reply::finish(vec![g]).say(format!("{label}: {} points", pts.len())),
                None => Reply::ask(label, save(&st)).refuse("points"),
            };
        }
        _ => {}
    }
    let preview = if st.points.len() >= 2 {
        vec![polyline(st.points.clone(), false)]
    } else {
        Vec::new()
    };
    Reply::ask(
        format!(
            "{label}: click points ({} so far), Enter to finish",
            st.points.len()
        ),
        save(&st),
    )
    .want(Want::Point)
    .preview(preview)
    .rubber(st.points.last().copied())
}

fn soft_edit(e: &Event) -> Reply {
    let flow = Flow::new("Soft Edit Curve")
        .number("Falloff distance along the curve", 5.0)
        .point("Point on the curve to move")
        .point("Point to move it to");
    let make = |c: &Collected, to: Option<Vec3>| -> Vec<(String, Geo)> {
        let (Some(fall), Some(from), Some(to)) = (c.n(0), c.p(0), c.p(1).or(to)) else {
            return Vec::new();
        };
        curves(e)
            .iter()
            .filter_map(|s| {
                let pts = resample(&dense(s, 400), s.closed, 80);
                let path = Path::new(&pts, s.closed)?;
                let s0 = path.closest(from);
                let d = sub(to, from);
                let moved: Vec<Vec3> = pts
                    .iter()
                    .map(|&p| {
                        let ds = (path.closest(p) - s0).abs();
                        let ds = if s.closed {
                            ds.min(path.length() - ds)
                        } else {
                            ds
                        };
                        let w = if ds >= fall {
                            0.0
                        } else {
                            0.5 + 0.5 * (std::f64::consts::PI * ds / fall).cos()
                        };
                        add(p, scale(d, w))
                    })
                    .collect();
                smooth(&moved, s.closed).map(|g| (s.id.clone(), g))
            })
            .collect()
    };
    flow.step(
        e,
        &|c, h| Show {
            geo: make(c, h).into_iter().map(|x| x.1).collect(),
            ..Default::default()
        },
        &|c| replace("Soft Edit Curve", make(c, None)),
    )
}

#[cfg(test)]
mod tests;

/// Helper for tests: a planar polygon offset keeps its distance.
pub fn fmt_len(v: f64) -> String {
    fmt(v)
}
