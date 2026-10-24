// SPDX-License-Identifier: LGPL-2.1-or-later
//! Matrix 9 **Curve** menu commands for OM9, written from the Matrix/Rhino behaviour
//! (prompts, command-line options, steps), not copied from it.
//!
//! Every command is a step function: OM9 sends the input, the command returns its new state,
//! its prompt and options, a preview and finally the curve to create. See the tests for the
//! step-by-step use of each command.
use om9_addon_abi::session::{
    self as s, Event, Geo, Input, Interactive, OptionToken, Reply, Vec3, Want, add, cross, dist,
    dot, fmt, norm, parse_number, scale, sub, unit,
};
use om9_addon_geom as g;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::f64::consts::{PI, TAU};

pub const POLYLINE: &str = "CurvePolylinePolyline";
pub const LINE: &str = "CurveLineSingleLine";
pub const INTERP: &str = "CurveFreeFormInterpolatePoints";
pub const CONTROL: &str = "CurveFreeFormControlPoints";
pub const RECTANGLE: &str = "CurveRectangleCornertoCorner";
pub const CIRCLE: &str = "CurveCircleCenterRadius";
pub const ELLIPSE: &str = "CurveEllipseFromCenter";
pub const ARC: &str = "CurveArcCenterStartAngle";
pub const ARC_SED: &str = "CurveArcStartEndDirection";
pub const POLYGON: &str = "CurvePolygonCenterRadius";
pub const SPIRAL: &str = "CurveSpiral";
pub const HELIX: &str = "CurveHelix";
pub const POINT: &str = "CurvePointObjectSinglePoint";

/// Points closer than this to the first point close a polyline (AutoClose), in mm.
pub const AUTO_CLOSE: f64 = 0.05;

pub struct Curves;

impl Interactive for Curves {
    const SESSION_COMMANDS: &'static [&'static str] = &[
        POLYLINE, LINE, INTERP, CONTROL, RECTANGLE, CIRCLE, ELLIPSE, ARC, ARC_SED, POLYGON, SPIRAL,
        HELIX, POINT,
    ];
    fn step(e: &Event) -> Result<Reply, String> {
        Ok(match e.command.as_str() {
            POLYLINE => polyline(e),
            LINE => line(e),
            INTERP => points_curve(e, PointsKind::Interpolate),
            CONTROL => points_curve(e, PointsKind::Control),
            RECTANGLE => rectangle(e),
            CIRCLE => circle(e),
            ELLIPSE => ellipse(e),
            ARC => arc(e),
            ARC_SED => arc_sed(e),
            POLYGON => polygon(e),
            SPIRAL => spiral(e, false),
            HELIX => spiral(e, true),
            POINT => point(e),
            other => return Err(format!("unknown command {other}")),
        })
    }
}

#[cfg(feature = "export")]
om9_addon_abi::export_interactive!(Curves);

// ---- helpers -------------------------------------------------------------------------------

fn load<T: DeserializeOwned + Default>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_default()
}
fn save<T: Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}
fn yes(v: &Option<String>, current: bool) -> bool {
    match v.as_deref().map(str::to_ascii_lowercase).as_deref() {
        Some("yes" | "y" | "true" | "1" | "on") => true,
        Some("no" | "n" | "false" | "0" | "off") => false,
        _ => !current,
    }
}
fn number_of(v: &Option<String>) -> Option<f64> {
    v.as_deref().and_then(parse_number)
}
/// The point projected on the active CPlane (through its origin moved to `base`'s height).
fn on_plane(e: &Event, base: Vec3, p: Vec3) -> Vec3 {
    let n = e.plane.normal();
    sub(p, scale(n, dot(sub(p, base), n)))
}
/// Unit direction in the CPlane from `a` to `b` (CPlane x when they coincide).
fn dir_in_plane(e: &Event, a: Vec3, b: Vec3) -> Vec3 {
    let d = sub(on_plane(e, a, b), a);
    if norm(d) < g::EPS { e.plane.x } else { unit(d) }
}

// ---- Polyline ------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct PolylineState {
    points: Vec<Vec3>,
    persistent_close: bool,
    length: Option<f64>,
}

impl PolylineState {
    /// Where a click or the cursor `p` puts the next point (Length constrains the distance).
    fn next(&self, p: Vec3) -> Vec3 {
        match (self.points.last(), self.length) {
            (Some(last), Some(len)) if dist(*last, p) > g::EPS => {
                add(*last, scale(unit(sub(p, *last)), len))
            }
            _ => p,
        }
    }
    fn reply(&self, hover: Option<Vec3>) -> Reply {
        let n = self.points.len();
        let mut opts = vec![OptionToken::toggle(
            "PersistentClose",
            self.persistent_close,
        )];
        if n >= 1 {
            opts.push(OptionToken::number("Length", self.length));
        }
        if n >= 3 {
            opts.push(OptionToken::action("Close"));
        }
        if n >= 1 {
            opts.push(OptionToken::action("Undo"));
        }
        let mut pts = self.points.clone();
        if let Some(h) = hover.filter(|_| n > 0) {
            pts.push(self.next(h));
        }
        let mut preview = Vec::new();
        if pts.len() >= 2 {
            preview.push(Geo::Polyline {
                points: pts.clone(),
                closed: self.persistent_close && pts.len() >= 3,
            });
        }
        let prompt = if n == 0 {
            "Start of polyline"
        } else {
            "Next point of polyline. Press Enter when done"
        };
        Reply::ask(prompt, save(self))
            .with_options(opts)
            .preview(preview)
            .rubber(None)
    }
    fn finish(&self, closed: bool) -> Reply {
        if self.points.len() < 2 {
            return Reply::finish(vec![]).say("Polyline cancelled: needs two points");
        }
        let closed = closed && self.points.len() >= 3;
        Reply::finish(vec![Geo::Polyline {
            points: self.points.clone(),
            closed,
        }])
        .say(format!(
            "Polyline: {} points{}",
            self.points.len(),
            if closed { ", closed" } else { "" }
        ))
    }
}

pub fn polyline(e: &Event) -> Reply {
    let mut st: PolylineState = load(&e.state);
    match &e.input {
        Input::Start | Input::Selection => st.reply(None),
        Input::Hover { point } => st.reply(Some(*point)),
        Input::Point { point } => {
            let p = st.next(*point);
            if st.points.len() >= 3 && dist(p, st.points[0]) <= AUTO_CLOSE {
                return st.finish(true);
            }
            if st.points.last().is_none_or(|l| dist(*l, p) > g::EPS) {
                st.points.push(p);
            }
            st.length = None;
            st.reply(None)
        }
        Input::Option { name, value } => match name.as_str() {
            "PersistentClose" => {
                st.persistent_close = yes(value, st.persistent_close);
                st.reply(None)
            }
            "Length" => match number_of(value) {
                Some(v) if v > 0.0 => {
                    st.length = Some(v);
                    st.reply(None)
                }
                _ => st.reply(None).refuse("Length must be a positive number"),
            },
            "Close" => st.finish(true),
            _ => st.reply(None).refuse(format!("Unknown option {name}")),
        },
        Input::Text { text } => match parse_number(text) {
            Some(v) if v > 0.0 && !st.points.is_empty() => {
                st.length = Some(v);
                st.reply(None)
            }
            _ => st
                .reply(None)
                .refuse(format!("Not a point or length: {text}")),
        },
        Input::Undo => {
            st.points.pop();
            st.reply(None)
        }
        Input::Enter => st.finish(st.persistent_close),
    }
}

// ---- Single Line ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct LineState {
    start: Option<Vec3>,
    both_sides: bool,
    length: Option<f64>,
}

impl LineState {
    fn segment(&self, p: Vec3) -> Option<(Vec3, Vec3)> {
        let a = self.start?;
        let d = sub(p, a);
        if norm(d) < g::EPS {
            return None;
        }
        let end = match self.length {
            Some(l) => add(a, scale(unit(d), if self.both_sides { l / 2.0 } else { l })),
            None => p,
        };
        let start = if self.both_sides {
            sub(scale(a, 2.0), end)
        } else {
            a
        };
        Some((start, end))
    }
    fn reply(&self, hover: Option<Vec3>) -> Reply {
        let prompt = if self.start.is_none() {
            if self.both_sides {
                "Middle of line"
            } else {
                "Start of line"
            }
        } else {
            "End of line"
        };
        let preview = hover
            .and_then(|h| self.segment(h))
            .map(|(a, b)| vec![Geo::Line { start: a, end: b }])
            .unwrap_or_default();
        Reply::ask(prompt, save(self))
            .with_options(vec![
                OptionToken::toggle("BothSides", self.both_sides),
                OptionToken::number("Length", self.length),
            ])
            .preview(preview)
    }
}

pub fn line(e: &Event) -> Reply {
    let mut st: LineState = load(&e.state);
    match &e.input {
        Input::Hover { point } => st.reply(Some(*point)),
        Input::Point { point } => {
            if st.start.is_none() {
                st.start = Some(*point);
                return st.reply(None);
            }
            match st.segment(*point) {
                Some((a, b)) => Reply::finish(vec![Geo::Line { start: a, end: b }])
                    .say(format!("Line: length {} mm", fmt(dist(a, b)))),
                None => st.reply(None).refuse("End point equals the start"),
            }
        }
        Input::Option { name, value } if name == "BothSides" => {
            st.both_sides = yes(value, st.both_sides);
            st.reply(None)
        }
        Input::Option { name, value } if name == "Length" => match number_of(value) {
            Some(v) if v > 0.0 => {
                st.length = Some(v);
                st.reply(None)
            }
            _ => st.reply(None).refuse("Length must be a positive number"),
        },
        Input::Text { text } => match parse_number(text) {
            Some(v) if v > 0.0 => {
                st.length = Some(v);
                st.reply(None)
            }
            _ => st
                .reply(None)
                .refuse(format!("Not a point or length: {text}")),
        },
        Input::Enter if st.start.is_none() => Reply::finish(vec![]),
        _ => st.reply(None),
    }
}

// ---- Interpolate / control-point curves ----------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum PointsKind {
    Interpolate,
    Control,
}

#[derive(Serialize, Deserialize)]
struct PointsState {
    points: Vec<Vec3>,
    degree: usize,
    persistent_close: bool,
}
impl Default for PointsState {
    fn default() -> Self {
        PointsState {
            points: Vec::new(),
            degree: 3,
            persistent_close: false,
        }
    }
}

fn build_points(kind: PointsKind, pts: &[Vec3], degree: usize, closed: bool) -> Option<Geo> {
    match kind {
        PointsKind::Interpolate => g::interpolate(pts, degree, closed),
        PointsKind::Control => {
            let mut p = pts.to_vec();
            if closed && p.len() > 2 {
                p.push(p[0]);
            }
            g::control_curve(&p, degree)
        }
    }
}

fn points_curve(e: &Event, kind: PointsKind) -> Reply {
    let mut st: PointsState = load(&e.state);
    let name = match kind {
        PointsKind::Interpolate => "interpolated curve",
        PointsKind::Control => "curve",
    };
    let reply = |st: &PointsState, hover: Option<Vec3>| {
        let mut pts = st.points.clone();
        if let Some(h) = hover.filter(|_| !pts.is_empty()) {
            pts.push(h);
        }
        let mut preview: Vec<Geo> = build_points(kind, &pts, st.degree, st.persistent_close)
            .into_iter()
            .collect();
        if kind == PointsKind::Control && pts.len() >= 2 {
            preview.push(Geo::Polyline {
                points: pts.clone(),
                closed: false,
            }); // control polygon
        }
        let mut opts = vec![
            OptionToken::number("Degree", Some(st.degree as f64)),
            OptionToken::toggle("PersistentClose", st.persistent_close),
        ];
        if st.points.len() >= 3 {
            opts.push(OptionToken::action("Close"));
        }
        if !st.points.is_empty() {
            opts.push(OptionToken::action("Undo"));
        }
        let prompt = if st.points.is_empty() {
            format!("Start of {name}")
        } else {
            "Next point. Press Enter when done".to_owned()
        };
        Reply::ask(prompt, save(st))
            .with_options(opts)
            .preview(preview)
    };
    let finish =
        |st: &PointsState, closed: bool| match build_points(kind, &st.points, st.degree, closed) {
            Some(c) => {
                Reply::finish(vec![c]).say(format!("Curve through {} points", st.points.len()))
            }
            None => Reply::finish(vec![]).say("Curve cancelled: needs two points"),
        };
    match &e.input {
        Input::Hover { point } => reply(&st, Some(*point)),
        Input::Point { point } => {
            if st.points.len() >= 3 && dist(*point, st.points[0]) <= AUTO_CLOSE {
                return finish(&st, true);
            }
            st.points.push(*point);
            reply(&st, None)
        }
        Input::Option { name, value } => match name.as_str() {
            "Degree" => match number_of(value) {
                Some(d) if (1.0..=11.0).contains(&d) && d.fract() == 0.0 => {
                    st.degree = d as usize;
                    reply(&st, None)
                }
                _ => reply(&st, None).refuse("Degree is a whole number from 1 to 11"),
            },
            "PersistentClose" => {
                st.persistent_close = yes(value, st.persistent_close);
                reply(&st, None)
            }
            "Close" => finish(&st, true),
            _ => reply(&st, None),
        },
        Input::Undo => {
            st.points.pop();
            reply(&st, None)
        }
        Input::Enter => finish(&st, st.persistent_close),
        _ => reply(&st, None),
    }
}

// ---- Rectangle -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct RectState {
    /// "corner" (default), "center", "3point".
    mode: String,
    vertical: bool,
    rounded: bool,
    points: Vec<Vec3>,
    length: Option<f64>,
    /// Corner radius asked after the rectangle is set (Rounded=Yes).
    rect: Option<[Vec3; 3]>,
}

impl RectState {
    fn mode(&self) -> &str {
        if self.mode.is_empty() {
            "corner"
        } else {
            &self.mode
        }
    }
    /// (corner, u, v) of the rectangle the clicked points and the cursor define.
    fn frame(&self, e: &Event, p: Vec3) -> Option<[Vec3; 3]> {
        let n = e.plane.normal();
        match (self.mode(), self.points.as_slice()) {
            ("corner", [a]) if self.vertical => {
                let q = on_plane(e, *a, p);
                let mut u = sub(q, *a);
                if let Some(l) = self.length {
                    u = scale(unit(u), l);
                }
                // Second edge goes up the CPlane normal; the click's height picks it later.
                Some([*a, u, scale(n, 0.0)])
            }
            ("corner", [a, b]) if self.vertical => {
                let h = dot(sub(p, *b), n);
                Some([*a, sub(*b, *a), scale(n, h)])
            }
            ("corner", [a]) => {
                let q = on_plane(e, *a, p);
                let d = e.plane.local(q);
                let o = e.plane.local(*a);
                let (mut du, dv) = (d[0] - o[0], d[1] - o[1]);
                if let Some(l) = self.length {
                    du = l * du.signum().max(if du == 0.0 { 1.0 } else { -1.0 });
                }
                Some([*a, scale(e.plane.x, du), scale(e.plane.y, dv)])
            }
            ("center", [c]) => {
                let q = on_plane(e, *c, p);
                let d = sub(q, *c);
                let (du, dv) = (dot(d, e.plane.x), dot(d, e.plane.y));
                let u = scale(e.plane.x, 2.0 * du);
                let v = scale(e.plane.y, 2.0 * dv);
                Some([sub(sub(*c, scale(u, 0.5)), scale(v, 0.5)), u, v])
            }
            ("3point", [a]) => Some([*a, sub(on_plane(e, *a, p), *a), [0.0; 3]]),
            ("3point", [a, b]) => {
                let u = sub(*b, *a);
                let side = unit(cross(n, u));
                let w = dot(sub(p, *a), side);
                Some([*a, u, scale(side, w)])
            }
            _ => None,
        }
    }
    fn geo(r: [Vec3; 3], radius: Option<f64>) -> Option<Geo> {
        let [o, u, v] = r;
        if norm(u) < g::EPS || norm(v) < g::EPS {
            return None;
        }
        match radius {
            Some(rad) if rad > g::EPS => g::rounded_rectangle(o, u, v, rad),
            _ => Some(Geo::Polyline {
                points: vec![o, add(o, u), add(add(o, u), v), add(o, v)],
                closed: true,
            }),
        }
    }
    fn needs(&self) -> usize {
        match (self.mode(), self.vertical) {
            ("3point", _) | ("corner", true) => 3,
            _ => 2,
        }
    }
    fn reply(&self, e: &Event, hover: Option<Vec3>) -> Reply {
        if let Some(r) = self.rect {
            let rad = hover.map(|h| {
                let corner = r[0];
                dist(on_plane(e, corner, h), corner).min(norm(r[1]).min(norm(r[2])) / 2.0)
            });
            return Reply::ask("Radius of the corners", save(self))
                .want(Want::Number)
                .preview(Self::geo(r, rad).into_iter().collect());
        }
        let preview = hover
            .and_then(|h| self.frame(e, h))
            .and_then(|r| {
                Self::geo(r, None).or(Some(Geo::Line {
                    start: r[0],
                    end: add(r[0], r[1]),
                }))
            })
            .into_iter()
            .collect();
        let prompt = match (self.mode(), self.points.len()) {
            ("center", 0) => "Center of rectangle",
            ("center", _) => "Corner of rectangle",
            ("3point", 0) => "Start of edge",
            ("3point", 1) => "End of edge",
            ("3point", _) => "Width",
            (_, 0) if self.vertical => "Start of edge",
            (_, 1) if self.vertical => "End of edge",
            (_, 2) => "Height",
            (_, 0) => "First corner of rectangle",
            _ => "Other corner or length",
        };
        let mut opts = Vec::new();
        if self.points.is_empty() {
            opts.extend([
                OptionToken::action("Center"),
                OptionToken::action("3Point"),
                OptionToken::toggle("Vertical", self.vertical),
            ]);
        }
        opts.push(OptionToken::toggle("Rounded", self.rounded));
        Reply::ask(prompt, save(self))
            .with_options(opts)
            .preview(preview)
    }
    fn complete(&mut self, e: &Event, r: [Vec3; 3]) -> Reply {
        if norm(r[1]) < g::EPS || norm(r[2]) < g::EPS {
            return self.reply(e, None).refuse("The rectangle has no area");
        }
        if self.rounded {
            self.rect = Some(r);
            return self.reply(e, None);
        }
        finish_rect(r, None)
    }
}

fn finish_rect(r: [Vec3; 3], radius: Option<f64>) -> Reply {
    match RectState::geo(r, radius) {
        Some(c) => Reply::finish(vec![c]).say(format!(
            "Rectangle {} × {} mm",
            fmt(norm(r[1])),
            fmt(norm(r[2]))
        )),
        None => Reply::finish(vec![]).say("Rectangle cancelled"),
    }
}

pub fn rectangle(e: &Event) -> Reply {
    let mut st: RectState = load(&e.state);
    match &e.input {
        Input::Hover { point } => st.reply(e, Some(*point)),
        Input::Point { point } => {
            if let Some(r) = st.rect {
                let rad = dist(on_plane(e, r[0], *point), r[0]);
                return finish_rect(r, Some(rad));
            }
            if st.points.len() + 1 < st.needs() {
                st.points.push(*point);
                return st.reply(e, None);
            }
            match st.frame(e, *point) {
                Some(r) => st.complete(e, r),
                None => st.reply(e, None),
            }
        }
        Input::Option { name, value } => {
            match name.as_str() {
                "Center" => st.mode = "center".into(),
                "3Point" => st.mode = "3point".into(),
                "Vertical" => st.vertical = yes(value, st.vertical),
                "Rounded" => st.rounded = yes(value, st.rounded),
                _ => {}
            }
            st.reply(e, None)
        }
        Input::Text { text } => {
            let Some(v) = parse_number(text).filter(|v| *v > 0.0) else {
                return st.reply(e, None).refuse(format!("Not a number: {text}"));
            };
            if let Some(r) = st.rect {
                if 2.0 * v > norm(r[1]).min(norm(r[2])) + g::EPS {
                    return st
                        .reply(e, None)
                        .refuse("Radius is larger than half the short side");
                }
                return finish_rect(r, Some(v));
            }
            match (st.mode(), st.points.as_slice()) {
                ("corner", [_]) if st.length.is_none() && !st.vertical => {
                    st.length = Some(v);
                    st.reply(e, None)
                }
                ("corner", [a]) if !st.vertical => {
                    let l = st.length.unwrap_or(v);
                    let r = [*a, scale(e.plane.x, l), scale(e.plane.y, v)];
                    st.complete(e, r)
                }
                _ => st.reply(e, None).refuse("Pick a point"),
            }
        }
        Input::Enter => Reply::finish(vec![]),
        _ => st.reply(e, None),
    }
}

// ---- Circle --------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct CircleState {
    /// "center" (default), "2point", "3point".
    mode: String,
    diameter: bool,
    vertical: bool,
    points: Vec<Vec3>,
}

impl CircleState {
    fn mode(&self) -> &str {
        if self.mode.is_empty() {
            "center"
        } else {
            &self.mode
        }
    }
    fn circle(&self, e: &Event, p: Vec3) -> Option<Geo> {
        let n = e.plane.normal();
        let (center, radius, x, y) = match (self.mode(), self.points.as_slice()) {
            ("center", [c]) if self.vertical => {
                let d = dir_in_plane(e, *c, p);
                (*c, dist(*c, p), d, n)
            }
            ("center", [c]) => {
                let q = on_plane(e, *c, p);
                (*c, dist(*c, q), e.plane.x, e.plane.y)
            }
            ("2point", [a]) => {
                let q = on_plane(e, *a, p);
                (
                    scale(add(*a, q), 0.5),
                    dist(*a, q) / 2.0,
                    e.plane.x,
                    e.plane.y,
                )
            }
            ("3point", [a, b]) => {
                let c = g::circumcenter(*a, *b, p)?;
                let nn = unit(cross(sub(*b, *a), sub(p, *a)));
                let x = unit(sub(*a, c));
                (c, dist(c, *a), x, cross(nn, x))
            }
            _ => return None,
        };
        (radius > g::EPS).then_some(Geo::Circle {
            center,
            x,
            y,
            radius,
        })
    }
    fn needs(&self) -> usize {
        if self.mode() == "3point" { 3 } else { 2 }
    }
    fn reply(&self, e: &Event, hover: Option<Vec3>) -> Reply {
        let prompt = match (self.mode(), self.points.len()) {
            ("center", 0) => "Center of circle",
            ("center", _) if self.diameter => "Diameter",
            ("center", _) => "Radius",
            ("2point", 0) => "Start of diameter",
            ("2point", _) => "End of diameter",
            ("3point", 0) => "First point on circle",
            ("3point", 1) => "Second point on circle",
            _ => "Third point on circle",
        };
        let mut opts = Vec::new();
        if self.points.is_empty() {
            opts.extend([
                OptionToken::action("2Point"),
                OptionToken::action("3Point"),
                OptionToken::toggle("Vertical", self.vertical),
            ]);
        } else if self.mode() == "center" {
            opts.push(OptionToken::choice(
                "Mode",
                if self.diameter { "Diameter" } else { "Radius" },
                &["Radius", "Diameter"],
            ));
        }
        let want = if self.mode() == "center" && !self.points.is_empty() {
            Want::Number
        } else {
            Want::Point
        };
        Reply::ask(prompt, save(self))
            .with_options(opts)
            .want(want)
            .preview(hover.and_then(|h| self.circle(e, h)).into_iter().collect())
            .rubber(
                self.points
                    .first()
                    .copied()
                    .filter(|_| self.mode() == "center"),
            )
    }
}

fn done_circle(c: Geo) -> Reply {
    let r = match &c {
        Geo::Circle { radius, .. } => *radius,
        _ => 0.0,
    };
    Reply::finish(vec![c]).say(format!(
        "Circle: radius {} mm, diameter {} mm",
        fmt(r),
        fmt(2.0 * r)
    ))
}

pub fn circle(e: &Event) -> Reply {
    let mut st: CircleState = load(&e.state);
    match &e.input {
        Input::Hover { point } => st.reply(e, Some(*point)),
        Input::Point { point } => {
            if st.points.len() + 1 < st.needs() {
                st.points.push(*point);
                return st.reply(e, None);
            }
            match st.circle(e, *point) {
                Some(c) => done_circle(c),
                None => st
                    .reply(e, None)
                    .refuse("The points do not define a circle"),
            }
        }
        Input::Option { name, value } => {
            match name.as_str() {
                "2Point" => st.mode = "2point".into(),
                "3Point" => st.mode = "3point".into(),
                "Vertical" => st.vertical = yes(value, st.vertical),
                "Mode" | "Diameter" | "Radius" => {
                    st.diameter = match value.as_deref() {
                        Some(v) => v.eq_ignore_ascii_case("diameter"),
                        None if name == "Diameter" => true,
                        None if name == "Radius" => false,
                        None => !st.diameter,
                    }
                }
                _ => {}
            }
            st.reply(e, None)
        }
        Input::Text { text } => match (parse_number(text), st.points.as_slice()) {
            (Some(v), [c]) if v > 0.0 && st.mode() == "center" => {
                let r = if st.diameter { v / 2.0 } else { v };
                let (x, y) = if st.vertical {
                    (e.plane.x, e.plane.normal())
                } else {
                    (e.plane.x, e.plane.y)
                };
                done_circle(Geo::Circle {
                    center: *c,
                    x,
                    y,
                    radius: r,
                })
            }
            _ => st.reply(e, None).refuse(format!("Not a radius: {text}")),
        },
        Input::Enter => Reply::finish(vec![]),
        _ => st.reply(e, None),
    }
}

// ---- Ellipse -------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct EllipseState {
    points: Vec<Vec3>,
}

fn ellipse_of(e: &Event, c: Vec3, a_end: Vec3, p: Vec3) -> Option<Geo> {
    let a = sub(on_plane(e, c, a_end), c);
    let side = unit(cross(e.plane.normal(), a));
    let b = dot(sub(p, c), side).abs();
    g::ellipse(c, a, scale(side, b))
}

pub fn ellipse(e: &Event) -> Reply {
    let mut st: EllipseState = load(&e.state);
    let reply = |st: &EllipseState, hover: Option<Vec3>| {
        let prompt = match st.points.len() {
            0 => "Ellipse center",
            1 => "End of first axis",
            _ => "End of second axis",
        };
        let preview: Vec<Geo> = match (st.points.as_slice(), hover) {
            ([c], Some(h)) => vec![Geo::Line {
                start: *c,
                end: on_plane(e, *c, h),
            }],
            ([c, a], Some(h)) => ellipse_of(e, *c, *a, h).into_iter().collect(),
            _ => vec![],
        };
        Reply::ask(prompt, save(st)).preview(preview)
    };
    match &e.input {
        Input::Hover { point } => reply(&st, Some(*point)),
        Input::Point { point } => {
            if st.points.len() < 2 {
                st.points.push(*point);
                return reply(&st, None);
            }
            match ellipse_of(e, st.points[0], st.points[1], *point) {
                Some(g) => Reply::finish(vec![g]).say("Ellipse"),
                None => reply(&st, None).refuse("Both axes need a length"),
            }
        }
        Input::Text { text } => match (parse_number(text), st.points.as_slice()) {
            (Some(v), [c, a]) if v > 0.0 => {
                let ax = sub(on_plane(e, *c, *a), *c);
                let side = unit(cross(e.plane.normal(), ax));
                match g::ellipse(*c, ax, scale(side, v)) {
                    Some(g) => Reply::finish(vec![g]).say("Ellipse"),
                    None => reply(&st, None).refuse("Both axes need a length"),
                }
            }
            (Some(v), [c]) if v > 0.0 => {
                st.points.push(add(*c, scale(e.plane.x, v)));
                reply(&st, None)
            }
            _ => reply(&st, None).refuse(format!("Not a length: {text}")),
        },
        Input::Enter => Reply::finish(vec![]),
        _ => reply(&st, None),
    }
}

// ---- Arc (center, start, angle) -------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct ArcState {
    points: Vec<Vec3>,
}

fn arc_from(e: &Event, c: Vec3, s0: Vec3, angle: f64) -> Option<Geo> {
    let x = sub(on_plane(e, c, s0), c);
    let r = norm(x);
    if r < g::EPS || angle.abs() < 1e-9 {
        return None;
    }
    let x = unit(x);
    let n = e.plane.normal();
    let y = if angle > 0.0 {
        cross(n, x)
    } else {
        cross(x, n)
    };
    Some(Geo::Arc {
        center: c,
        x,
        y,
        radius: r,
        angle: angle.abs().min(TAU),
    })
}

fn polar(e: &Event, c: Vec3, s0: Vec3, p: Vec3) -> f64 {
    let x = unit(sub(on_plane(e, c, s0), c));
    let y = cross(e.plane.normal(), x);
    let d = sub(p, c);
    let a = dot(d, y).atan2(dot(d, x));
    if a <= 0.0 { a + TAU } else { a }
}

pub fn arc(e: &Event) -> Reply {
    let mut st: ArcState = load(&e.state);
    let reply = |st: &ArcState, hover: Option<Vec3>| {
        let prompt = match st.points.len() {
            0 => "Center of arc",
            1 => "Start of arc",
            _ => "End point or angle",
        };
        let preview: Vec<Geo> = match (st.points.as_slice(), hover) {
            ([c], Some(h)) => vec![Geo::Circle {
                center: *c,
                x: e.plane.x,
                y: e.plane.y,
                radius: dist(*c, on_plane(e, *c, h)).max(1e-6),
            }],
            ([c, s0], Some(h)) => arc_from(e, *c, *s0, polar(e, *c, *s0, h))
                .into_iter()
                .collect(),
            _ => vec![],
        };
        Reply::ask(prompt, save(st))
            .preview(preview)
            .rubber(st.points.first().copied())
            .want(if st.points.len() == 2 {
                Want::Number
            } else {
                Want::Point
            })
    };
    let done = |g: Geo| {
        let (r, a) = match &g {
            Geo::Arc { radius, angle, .. } => (*radius, *angle),
            _ => (0.0, 0.0),
        };
        Reply::finish(vec![g]).say(format!(
            "Arc: radius {} mm, angle {}°, length {} mm",
            fmt(r),
            fmt(a.to_degrees()),
            fmt(r * a)
        ))
    };
    match &e.input {
        Input::Hover { point } => reply(&st, Some(*point)),
        Input::Point { point } => {
            if st.points.len() < 2 {
                st.points.push(*point);
                return reply(&st, None);
            }
            match arc_from(
                e,
                st.points[0],
                st.points[1],
                polar(e, st.points[0], st.points[1], *point),
            ) {
                Some(g) => done(g),
                None => reply(&st, None).refuse("The arc needs a radius and an angle"),
            }
        }
        Input::Text { text } => match (parse_number(text), st.points.as_slice()) {
            (Some(deg), [c, s0]) => match arc_from(e, *c, *s0, deg.to_radians()) {
                Some(g) => done(g),
                None => reply(&st, None).refuse("The angle must not be 0"),
            },
            (Some(r), [c]) if r > 0.0 => {
                st.points.push(add(*c, scale(e.plane.x, r)));
                reply(&st, None)
            }
            _ => reply(&st, None).refuse(format!("Not a number: {text}")),
        },
        Input::Enter => Reply::finish(vec![]),
        _ => reply(&st, None),
    }
}

/// Arc from start, end and the direction at the start.
pub fn arc_sed(e: &Event) -> Reply {
    let mut st: ArcState = load(&e.state);
    let reply = |st: &ArcState, hover: Option<Vec3>| {
        let prompt = match st.points.len() {
            0 => "Start of arc",
            1 => "End of arc",
            _ => "Direction at start",
        };
        let preview: Vec<Geo> = match (st.points.as_slice(), hover) {
            ([a], Some(h)) => vec![Geo::Line { start: *a, end: h }],
            ([a, b], Some(h)) => g::arc_start_end_dir(*a, *b, sub(h, *a))
                .into_iter()
                .collect(),
            _ => vec![],
        };
        Reply::ask(prompt, save(st))
            .preview(preview)
            .rubber(st.points.first().copied())
    };
    match &e.input {
        Input::Hover { point } => reply(&st, Some(*point)),
        Input::Point { point } => {
            if st.points.len() < 2 {
                st.points.push(*point);
                return reply(&st, None);
            }
            match g::arc_start_end_dir(st.points[0], st.points[1], sub(*point, st.points[0])) {
                Some(g) => Reply::finish(vec![g]).say("Arc"),
                None => reply(&st, None).refuse("Pick a direction away from the start"),
            }
        }
        Input::Enter => Reply::finish(vec![]),
        _ => reply(&st, None),
    }
}

// ---- Polygon -------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct PolygonState {
    center: Option<Vec3>,
    sides: usize,
    circumscribed: bool,
}
impl Default for PolygonState {
    fn default() -> Self {
        PolygonState {
            center: None,
            sides: 5,
            circumscribed: false,
        }
    }
}

fn polygon_points(
    e: &Event,
    c: Vec3,
    corner: Vec3,
    sides: usize,
    circumscribed: bool,
) -> Option<Vec<Vec3>> {
    let d = sub(on_plane(e, c, corner), c);
    let r = norm(d);
    if r < g::EPS || sides < 3 {
        return None;
    }
    let x = unit(d);
    let y = cross(e.plane.normal(), x);
    // Circumscribed: the picked point is the middle of an edge.
    let (rv, start) = if circumscribed {
        (r / (PI / sides as f64).cos(), PI / sides as f64)
    } else {
        (r, 0.0)
    };
    Some(
        (0..sides)
            .map(|i| {
                let a = start + TAU * i as f64 / sides as f64;
                add(c, add(scale(x, rv * a.cos()), scale(y, rv * a.sin())))
            })
            .collect(),
    )
}

pub fn polygon(e: &Event) -> Reply {
    let mut st: PolygonState = load(&e.state);
    let reply = |st: &PolygonState, hover: Option<Vec3>| {
        let preview = match (st.center, hover) {
            (Some(c), Some(h)) => polygon_points(e, c, h, st.sides, st.circumscribed)
                .map(|p| {
                    vec![Geo::Polyline {
                        points: p,
                        closed: true,
                    }]
                })
                .unwrap_or_default(),
            _ => vec![],
        };
        Reply::ask(
            if st.center.is_none() {
                "Center of polygon"
            } else if st.circumscribed {
                "Midpoint of edge"
            } else {
                "Corner of polygon"
            },
            save(st),
        )
        .with_options(vec![
            OptionToken::number("NumSides", Some(st.sides as f64)),
            OptionToken::toggle("Circumscribed", st.circumscribed),
        ])
        .preview(preview)
        .rubber(st.center)
    };
    let finish = |st: &PolygonState, p: Vec3| match st
        .center
        .and_then(|c| polygon_points(e, c, p, st.sides, st.circumscribed))
    {
        Some(pts) => Reply::finish(vec![Geo::Polyline {
            points: pts,
            closed: true,
        }])
        .say(format!("Polygon: {} sides", st.sides)),
        None => reply(st, None).refuse("The polygon needs a size"),
    };
    match &e.input {
        Input::Hover { point } => reply(&st, Some(*point)),
        Input::Point { point } => {
            if st.center.is_none() {
                st.center = Some(*point);
                return reply(&st, None);
            }
            finish(&st, *point)
        }
        Input::Option { name, value } => {
            match name.as_str() {
                "NumSides" => match number_of(value) {
                    Some(n) if n >= 3.0 && n <= 1000.0 && n.fract() == 0.0 => st.sides = n as usize,
                    _ => return reply(&st, None).refuse("NumSides is a whole number ≥ 3"),
                },
                "Circumscribed" => st.circumscribed = yes(value, st.circumscribed),
                _ => {}
            }
            reply(&st, None)
        }
        Input::Text { text } => match (parse_number(text), st.center) {
            (Some(r), Some(c)) if r > 0.0 => finish(&st, add(c, scale(e.plane.x, r))),
            _ => reply(&st, None).refuse(format!("Not a radius: {text}")),
        },
        Input::Enter => Reply::finish(vec![]),
        _ => reply(&st, None),
    }
}

// ---- Spiral / Helix ------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct SpiralState {
    points: Vec<Vec3>,
    flat: bool,
    turns: f64,
    r0: Option<f64>,
}
impl Default for SpiralState {
    fn default() -> Self {
        SpiralState {
            points: Vec::new(),
            flat: false,
            turns: 10.0,
            r0: None,
        }
    }
}

const PER_TURN: usize = 16;

fn spiral_geo(e: &Event, st: &SpiralState, helix: bool, r_point: Vec3) -> Option<Geo> {
    let a = *st.points.first()?;
    let b = if st.flat { a } else { *st.points.get(1)? };
    let axis = sub(b, a);
    let n = if norm(axis) > g::EPS {
        unit(axis)
    } else {
        e.plane.normal()
    };
    let d = sub(r_point, a);
    let radial = sub(d, scale(n, dot(d, n)));
    let r = norm(radial);
    if r < g::EPS {
        return None;
    }
    let (r0, r1) = if helix {
        (r, r)
    } else {
        (st.r0.unwrap_or(r), r)
    };
    let start = if helix || st.r0.is_none() {
        radial
    } else {
        e.plane.x
    };
    let pts = g::spiral_points(a, b, e.plane.normal(), start, r0, r1, st.turns, PER_TURN);
    g::interpolate(&pts, 3, false)
}

pub fn spiral(e: &Event, helix: bool) -> Reply {
    let mut st: SpiralState = load(&e.state);
    // Points needed before the radius: axis start (+ end unless flat).
    let axis_points = if st.flat && !helix { 1 } else { 2 };
    let reply = |st: &SpiralState, hover: Option<Vec3>| {
        let stage = st.points.len();
        let prompt = if stage == 0 {
            "Start of axis"
        } else if stage < axis_points {
            "End of axis"
        } else if helix {
            "Radius"
        } else if st.r0.is_none() {
            "First radius"
        } else {
            "Second radius"
        };
        let preview: Vec<Geo> = match hover {
            Some(h) if stage >= axis_points => spiral_geo(e, st, helix, h).into_iter().collect(),
            Some(h) if stage == 1 => vec![Geo::Line {
                start: st.points[0],
                end: h,
            }],
            _ => vec![],
        };
        let mut opts = vec![OptionToken::number("Turns", Some(st.turns))];
        if !helix && stage <= 1 {
            opts.push(OptionToken::toggle("Flat", st.flat));
        }
        Reply::ask(prompt, save(st))
            .with_options(opts)
            .preview(preview)
            .want(if stage >= axis_points {
                Want::Number
            } else {
                Want::Point
            })
    };
    let radius_done = |st: &mut SpiralState, p: Vec3| -> Reply {
        if !helix && st.r0.is_none() {
            let a = st.points[0];
            let b = if st.flat { a } else { st.points[1] };
            let n = if dist(a, b) > g::EPS {
                unit(sub(b, a))
            } else {
                e.plane.normal()
            };
            let d = sub(p, a);
            st.r0 = Some(norm(sub(d, scale(n, dot(d, n)))));
            return reply(st, None);
        }
        match spiral_geo(e, st, helix, p) {
            Some(g) => Reply::finish(vec![g]).say(format!(
                "{}: {} turns",
                if helix { "Helix" } else { "Spiral" },
                fmt(st.turns)
            )),
            None => reply(st, None).refuse("The radius must not be 0"),
        }
    };
    match &e.input {
        Input::Hover { point } => reply(&st, Some(*point)),
        Input::Point { point } => {
            if st.points.len() < axis_points {
                st.points.push(*point);
                return reply(&st, None);
            }
            radius_done(&mut st, *point)
        }
        Input::Option { name, value } => {
            match name.as_str() {
                "Turns" => match number_of(value) {
                    Some(t) if t > 0.0 && t <= 1000.0 => st.turns = t,
                    _ => return reply(&st, None).refuse("Turns must be positive"),
                },
                "Flat" => st.flat = yes(value, st.flat),
                _ => {}
            }
            reply(&st, None)
        }
        Input::Text { text } => match parse_number(text) {
            Some(r) if r > 0.0 && st.points.len() >= axis_points => {
                let a = st.points[0];
                let b = if st.flat { a } else { st.points[1] };
                let n = if dist(a, b) > g::EPS {
                    unit(sub(b, a))
                } else {
                    e.plane.normal()
                };
                let x = unit(sub(e.plane.x, scale(n, dot(e.plane.x, n))));
                let x = if norm(x) < 0.5 { e.plane.y } else { x };
                radius_done(&mut st, add(a, scale(x, r)))
            }
            _ => reply(&st, None).refuse(format!("Not a radius: {text}")),
        },
        Input::Enter if !helix && st.points.len() == 1 => {
            // Enter at "End of axis": a flat spiral in the CPlane.
            st.flat = true;
            reply(&st, None)
        }
        Input::Enter => Reply::finish(vec![]),
        _ => reply(&st, None),
    }
}

// ---- Point ---------------------------------------------------------------------------------

pub fn point(e: &Event) -> Reply {
    match &e.input {
        Input::Point { point } => Reply::finish(vec![Geo::Point { point: *point }]).say(format!(
            "Point {}, {}, {}",
            fmt(point[0]),
            fmt(point[1]),
            fmt(point[2])
        )),
        Input::Enter => Reply::finish(vec![]),
        _ => Reply::ask("Location of point object", Value::Null),
    }
}

/// Test helper: an event for `command` with `input` after `state`.
pub fn event(command: &str, input: Input, state: &Value) -> Event {
    Event {
        command: command.into(),
        input,
        state: state.clone(),
        plane: s::Plane::default(),
        selection: Vec::new(),
        settings: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run a command through a list of inputs; returns every reply.
    fn run(command: &str, inputs: Vec<Input>) -> Vec<Reply> {
        let mut state = Value::Null;
        let mut out = Vec::new();
        for input in std::iter::once(Input::Start).chain(inputs) {
            let hover = matches!(input, Input::Hover { .. });
            let r = Curves::step(&event(command, input, &state)).unwrap();
            if !hover {
                state = r.state.clone();
            }
            out.push(r);
        }
        out
    }
    fn pt(x: f64, y: f64) -> Input {
        Input::Point { point: [x, y, 0.0] }
    }
    fn opt(name: &str, value: Option<&str>) -> Input {
        Input::Option {
            name: name.into(),
            value: value.map(str::to_owned),
        }
    }
    fn text(t: &str) -> Input {
        Input::Text { text: t.into() }
    }

    #[test]
    fn polyline_collects_points_with_undo_length_and_autoclose() {
        let r = run(
            POLYLINE,
            vec![
                pt(0.0, 0.0),
                pt(10.0, 0.0),
                Input::Hover {
                    point: [10.0, 5.0, 0.0],
                },
            ],
        );
        assert_eq!(r[0].prompt, "Start of polyline");
        assert!(r[2].options.iter().any(|o| o.name == "Length"));
        assert!(
            !r[2].options.iter().any(|o| o.name == "Close"),
            "Close needs three points"
        );
        let Geo::Polyline { points, .. } = &r[3].preview[0] else {
            panic!()
        };
        assert_eq!(points.len(), 3, "rubber segment to the cursor");
        // Length=5 constrains the next segment's distance, keeping the clicked direction.
        let r = run(
            POLYLINE,
            vec![
                pt(0.0, 0.0),
                opt("Length", Some("5")),
                pt(100.0, 0.0),
                Input::Enter,
            ],
        );
        let last = r.last().unwrap();
        assert!(last.done);
        let Geo::Polyline { points, closed } = &last.create[0] else {
            panic!()
        };
        assert_eq!(points[1], [5.0, 0.0, 0.0]);
        assert!(!closed);
        // Undo removes the last point; clicking near the start closes it (AutoClose).
        let r = run(
            POLYLINE,
            vec![
                pt(0.0, 0.0),
                pt(10.0, 0.0),
                pt(99.0, 99.0),
                Input::Undo,
                pt(10.0, 10.0),
                pt(0.0, 0.01),
            ],
        );
        let last = r.last().unwrap();
        assert!(last.done);
        let Geo::Polyline { points, closed } = &last.create[0] else {
            panic!()
        };
        assert_eq!(points.len(), 3);
        assert!(*closed);
        // One point + Enter creates nothing.
        let r = run(POLYLINE, vec![pt(0.0, 0.0), Input::Enter]);
        assert!(r.last().unwrap().done && r.last().unwrap().create.is_empty());
        // A bad length is refused and the command keeps going.
        let r = run(POLYLINE, vec![pt(0.0, 0.0), opt("Length", Some("-1"))]);
        assert!(r.last().unwrap().error.is_some() && !r.last().unwrap().done);
    }

    #[test]
    fn single_line_with_both_sides_and_length() {
        let r = run(
            LINE,
            vec![
                opt("BothSides", None),
                pt(0.0, 0.0),
                opt("Length", Some("10")),
                pt(3.0, 0.0),
            ],
        );
        let Geo::Line { start, end } = r.last().unwrap().create[0] else {
            panic!()
        };
        assert_eq!((start, end), ([-5.0, 0.0, 0.0], [5.0, 0.0, 0.0]));
    }

    #[test]
    fn interpolated_and_control_curves() {
        let r = run(
            INTERP,
            vec![
                pt(0.0, 0.0),
                pt(5.0, 5.0),
                pt(10.0, 0.0),
                pt(15.0, 5.0),
                Input::Enter,
            ],
        );
        let c = &r.last().unwrap().create[0];
        assert!(matches!(c, Geo::Nurbs { degree: 3, .. }));
        for t in [0.0, 1.0] {
            let p = g::eval_geo(c, t).unwrap();
            assert!(dist(p, if t == 0.0 { [0.0; 3] } else { [15.0, 5.0, 0.0] }) < 1e-9);
        }
        let r = run(
            CONTROL,
            vec![
                opt("Degree", Some("2")),
                pt(0.0, 0.0),
                pt(5.0, 5.0),
                pt(10.0, 0.0),
                Input::Enter,
            ],
        );
        assert!(
            matches!(&r.last().unwrap().create[0], Geo::Nurbs { degree: 2, points, .. } if points.len() == 3)
        );
        let r = run(CONTROL, vec![opt("Degree", Some("2.5"))]);
        assert!(r.last().unwrap().error.is_some());
    }

    #[test]
    fn rectangle_modes() {
        let corners = |g: &Geo| match g {
            Geo::Polyline {
                points,
                closed: true,
            } => points.clone(),
            _ => panic!("{g:?}"),
        };
        let r = run(RECTANGLE, vec![pt(1.0, 1.0), pt(11.0, 7.0)]);
        assert_eq!(
            corners(&r[2].create[0]),
            vec![
                [1.0, 1.0, 0.0],
                [11.0, 1.0, 0.0],
                [11.0, 7.0, 0.0],
                [1.0, 7.0, 0.0]
            ]
        );
        assert!(r[2].messages[0].contains("10 × 6"));
        let r = run(
            RECTANGLE,
            vec![opt("Center", None), pt(0.0, 0.0), pt(5.0, 2.0)],
        );
        assert_eq!(corners(&r[3].create[0])[0], [-5.0, -2.0, 0.0]);
        // Typed length then width.
        let r = run(RECTANGLE, vec![pt(0.0, 0.0), text("8"), text("3")]);
        assert_eq!(corners(&r[3].create[0])[2], [8.0, 3.0, 0.0]);
        // 3Point: a slanted rectangle.
        let r = run(
            RECTANGLE,
            vec![
                opt("3Point", None),
                pt(0.0, 0.0),
                pt(3.0, 4.0),
                pt(-4.0, 3.0),
            ],
        );
        let c = corners(&r[4].create[0]);
        assert!(dist(c[3], [-4.0, 3.0, 0.0]) < 1e-9);
        // Vertical: edge on the CPlane, height along its normal.
        let r = run(
            RECTANGLE,
            vec![
                opt("Vertical", None),
                pt(0.0, 0.0),
                pt(4.0, 0.0),
                Input::Point {
                    point: [4.0, 0.0, 2.0],
                },
            ],
        );
        assert_eq!(corners(&r[4].create[0])[2], [4.0, 0.0, 2.0]);
        // Rounded: the radius comes after the corners.
        let r = run(
            RECTANGLE,
            vec![opt("Rounded", None), pt(0.0, 0.0), pt(10.0, 6.0), text("1")],
        );
        assert!(matches!(&r[4].create[0], Geo::Nurbs { degree: 2, .. }));
        let r = run(
            RECTANGLE,
            vec![opt("Rounded", None), pt(0.0, 0.0), pt(10.0, 6.0), text("4")],
        );
        assert!(
            r[4].error.is_some(),
            "radius larger than half the short side"
        );
    }

    #[test]
    fn circle_modes() {
        let radius = |g: &Geo| match g {
            Geo::Circle { radius, center, .. } => (*center, *radius),
            _ => panic!(),
        };
        let r = run(CIRCLE, vec![pt(1.0, 1.0), pt(4.0, 5.0)]);
        assert_eq!(radius(&r[2].create[0]), ([1.0, 1.0, 0.0], 5.0));
        let r = run(
            CIRCLE,
            vec![pt(0.0, 0.0), opt("Mode", Some("Diameter")), text("17.35")],
        );
        assert!(
            (radius(&r[3].create[0]).1 - 8.675).abs() < 1e-12,
            "ring size typed as a diameter"
        );
        let r = run(
            CIRCLE,
            vec![opt("2Point", None), pt(0.0, 0.0), pt(6.0, 0.0)],
        );
        assert_eq!(radius(&r[3].create[0]), ([3.0, 0.0, 0.0], 3.0));
        let r = run(
            CIRCLE,
            vec![
                opt("3Point", None),
                pt(2.0, 0.0),
                pt(0.0, 2.0),
                pt(-2.0, 0.0),
            ],
        );
        let (c, rr) = radius(&r[4].create[0]);
        assert!(dist(c, [0.0; 3]) < 1e-12 && (rr - 2.0).abs() < 1e-12);
        let r = run(
            CIRCLE,
            vec![
                opt("3Point", None),
                pt(0.0, 0.0),
                pt(1.0, 0.0),
                pt(2.0, 0.0),
            ],
        );
        assert!(r[4].error.is_some(), "collinear points");
        let r = run(
            CIRCLE,
            vec![opt("Vertical", None), pt(0.0, 0.0), pt(3.0, 0.0)],
        );
        let Geo::Circle { y, .. } = r[3].create[0] else {
            panic!()
        };
        assert_eq!(
            y,
            [0.0, 0.0, 1.0],
            "vertical circle stands on the CPlane normal"
        );
    }

    #[test]
    fn ellipse_arc_polygon() {
        let r = run(ELLIPSE, vec![pt(0.0, 0.0), pt(4.0, 0.0), pt(1.0, 2.0)]);
        let e0 = &r[3].create[0];
        assert!(dist(g::eval_geo(e0, 0.25).unwrap(), [0.0, 2.0, 0.0]) < 1e-9);
        // Arc: center, start, 90° typed.
        let r = run(ARC, vec![pt(0.0, 0.0), pt(3.0, 0.0), text("90")]);
        let a = &r[3].create[0];
        assert!(dist(g::eval_geo(a, 1.0).unwrap(), [0.0, 3.0, 0.0]) < 1e-9);
        // Negative angle goes clockwise.
        let r = run(ARC, vec![pt(0.0, 0.0), pt(3.0, 0.0), text("-90")]);
        assert!(dist(g::eval_geo(&r[3].create[0], 1.0).unwrap(), [0.0, -3.0, 0.0]) < 1e-9);
        // End point picks the angle counter-clockwise from the start.
        let r = run(ARC, vec![pt(0.0, 0.0), pt(3.0, 0.0), pt(-1.0, 0.0)]);
        let Geo::Arc { angle, .. } = r[3].create[0] else {
            panic!()
        };
        assert!((angle - PI).abs() < 1e-12);
        let r = run(ARC_SED, vec![pt(2.0, 0.0), pt(0.0, 2.0), pt(2.0, 1.0)]);
        assert!(dist(g::eval_geo(&r[3].create[0], 1.0).unwrap(), [0.0, 2.0, 0.0]) < 1e-9);
        // Hexagon with the corner on +x; circumscribed puts the click on an edge middle.
        let r = run(
            POLYGON,
            vec![opt("NumSides", Some("6")), pt(0.0, 0.0), pt(2.0, 0.0)],
        );
        let Geo::Polyline {
            points,
            closed: true,
        } = &r[3].create[0]
        else {
            panic!()
        };
        assert_eq!(points.len(), 6);
        assert!(points.iter().all(|p| (norm(*p) - 2.0).abs() < 1e-12));
        let r = run(
            POLYGON,
            vec![
                opt("Circumscribed", None),
                opt("NumSides", Some("4")),
                pt(0.0, 0.0),
                pt(1.0, 0.0),
            ],
        );
        let Geo::Polyline { points, .. } = &r[4].create[0] else {
            panic!()
        };
        assert!(
            points
                .iter()
                .all(|p| (norm(*p) - 2f64.sqrt()).abs() < 1e-12),
            "square around a unit circle"
        );
    }

    #[test]
    fn helix_and_flat_spiral() {
        let r = run(
            HELIX,
            vec![
                opt("Turns", Some("3")),
                pt(0.0, 0.0),
                Input::Point {
                    point: [0.0, 0.0, 6.0],
                },
                pt(2.0, 0.0),
            ],
        );
        let h = &r[4].create[0];
        assert!(dist(g::eval_geo(h, 1.0).unwrap(), [2.0, 0.0, 6.0]) < 1e-6);
        assert!(r[4].messages[0].contains("3 turns"));
        let r = run(
            SPIRAL,
            vec![
                opt("Flat", None),
                opt("Turns", Some("2")),
                pt(0.0, 0.0),
                pt(1.0, 0.0),
                pt(5.0, 0.0),
            ],
        );
        let sp = &r[5].create[0];
        for i in 0..=20 {
            assert!(
                g::eval_geo(sp, i as f64 / 20.0).unwrap()[2].abs() < 1e-9,
                "flat"
            );
        }
        assert!(dist(g::eval_geo(sp, 1.0).unwrap(), [5.0, 0.0, 0.0]) < 1e-6);
        let r = run(POINT, vec![pt(1.0, 2.0)]);
        assert_eq!(
            r[1].create[0],
            Geo::Point {
                point: [1.0, 2.0, 0.0]
            }
        );
    }
}
