// SPDX-License-Identifier: LGPL-2.1-or-later
//! Small helpers for writing session commands: state in and out, the stones and curves of the
//! selection, and the common "select objects → preview → Enter" flow.
use crate::gems::PlacedGem;
use crate::mesh::Frame;
use om9_addon_abi::session::{Event, Geo, Input, OptionToken, Reply, Selected, Vec3, Want};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// The command's state from the last reply (default on start or when unreadable).
pub fn load<T: DeserializeOwned + Default>(e: &Event) -> T {
    serde_json::from_value(e.state.clone()).unwrap_or_default()
}

pub fn save<T: Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}

/// The CPlane at `p` as a frame (z = CPlane normal, x = CPlane x).
pub fn plane_frame(e: &Event, p: Vec3) -> Frame {
    Frame::new(p, e.plane.normal(), e.plane.x)
}

/// The stones of the selection, and how many selected objects are not stones.
pub fn selected_gems(e: &Event) -> (Vec<PlacedGem>, usize) {
    let mut gems = Vec::new();
    let mut other = 0;
    for s in &e.selection {
        match PlacedGem::from_selected(s, e.plane.normal()) {
            Some(g) => gems.push(g),
            None => other += 1,
        }
    }
    (gems, other)
}

/// The curves of the selection (samples in world mm).
pub fn selected_curves(e: &Event) -> Vec<&Selected> {
    e.selection
        .iter()
        .filter(|s| s.kind == "curve" && s.points.len() >= 2)
        .collect()
}

/// `(n other objects ignored)` or nothing.
pub fn ignored(n: usize) -> String {
    if n > 0 {
        format!(" ({n} other objects ignored)")
    } else {
        String::new()
    }
}

/// What a selection command builds right now.
pub struct Built {
    pub create: Vec<Geo>,
    /// Shown in the prompt and the history: `12 prongs`.
    pub summary: String,
    pub preview: Vec<Geo>,
}

/// The usual flow of a command that works on the selection: ask for objects until `build`
/// has something to make, preview it, create it on Enter.
pub fn selection_command(
    e: &Event,
    label: &str,
    select_prompt: &str,
    build: impl FnOnce(&Event) -> Result<Built, String>,
) -> Reply {
    let built = match build(e) {
        Ok(b) => b,
        Err(why) => {
            let r = Reply::ask(format!("{label}: {select_prompt}"), json!({})).want(Want::Objects);
            return if matches!(e.input, Input::Enter) {
                r.refuse(why)
            } else {
                r
            };
        }
    };
    if built.create.is_empty() {
        let r = Reply::ask(format!("{label}: {select_prompt}"), json!({})).want(Want::Objects);
        return if matches!(e.input, Input::Enter) {
            r.refuse(format!("{label}: nothing to create. {}", built.summary))
        } else {
            r
        };
    }
    if matches!(e.input, Input::Enter) {
        return Reply::finish(built.create).say(format!("{label}: {}", built.summary));
    }
    let preview = if built.preview.is_empty() {
        built.create
    } else {
        built.preview
    };
    Reply::ask(
        format!("{label}: {}. Adjust the options, Enter to create", built.summary),
        json!({}),
    )
    .want(Want::Objects)
    .preview(preview)
}

/// A closed polyline through world points (for outlines and guides).
pub fn closed_polyline(points: Vec<Vec3>) -> Geo {
    Geo::Polyline {
        points,
        closed: true,
    }
}

// ---- step-by-step flows --------------------------------------------------------------------

/// One thing a command asks for, in order.
#[derive(Debug, Clone)]
pub enum Need {
    /// A clicked or typed point, with its prompt.
    Point(String),
    /// A typed number (Enter takes the default), with its prompt.
    Number(String, f64),
}

/// A collected answer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum Answer {
    Point(Vec3),
    Number(f64),
}

/// What a flow has collected so far.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Collected {
    pub selected: bool,
    pub answers: Vec<Answer>,
    pub options: std::collections::BTreeMap<String, String>,
}

impl Collected {
    /// The k-th point answer.
    pub fn p(&self, k: usize) -> Option<Vec3> {
        self.answers
            .iter()
            .filter_map(|a| match a {
                Answer::Point(p) => Some(*p),
                _ => None,
            })
            .nth(k)
    }
    /// The k-th number answer.
    pub fn n(&self, k: usize) -> Option<f64> {
        self.answers
            .iter()
            .filter_map(|a| match a {
                Answer::Number(v) => Some(*v),
                _ => None,
            })
            .nth(k)
    }
    pub fn points(&self) -> Vec<Vec3> {
        (0..).map_while(|k| self.p(k)).collect()
    }
    /// An option's value (its default until changed).
    pub fn opt<'a>(&'a self, spec: &'a Flow, name: &str) -> &'a str {
        self.options
            .get(name)
            .map(String::as_str)
            .or_else(|| spec.options.iter().find(|o| o.0 == name).map(|o| o.2.as_str()))
            .unwrap_or("")
    }
    pub fn yes(&self, spec: &Flow, name: &str) -> bool {
        self.opt(spec, name).eq_ignore_ascii_case("yes")
    }
}

/// A command flow: optionally objects first, then points and numbers in order, with
/// command-line options (`(name, choices, default)`; `["Yes", "No"]` for a toggle).
pub struct Flow {
    pub label: String,
    pub select: bool,
    pub needs: Vec<Need>,
    pub options: Vec<(String, Vec<String>, String)>,
}

/// What a flow shows while it runs.
#[derive(Default)]
pub struct Show {
    pub geo: Vec<Geo>,
    pub transforms: Vec<[f64; 16]>,
    pub rubber: Option<Vec3>,
}

impl Flow {
    pub fn new(label: &str) -> Flow {
        Flow {
            label: label.to_owned(),
            select: true,
            needs: Vec::new(),
            options: Vec::new(),
        }
    }
    pub fn no_selection(mut self) -> Flow {
        self.select = false;
        self
    }
    pub fn point(mut self, prompt: &str) -> Flow {
        self.needs.push(Need::Point(prompt.to_owned()));
        self
    }
    pub fn number(mut self, prompt: &str, default: f64) -> Flow {
        self.needs.push(Need::Number(prompt.to_owned(), default));
        self
    }
    pub fn toggle(mut self, name: &str, on: bool) -> Flow {
        self.options.push((
            name.to_owned(),
            vec!["Yes".into(), "No".into()],
            if on { "Yes" } else { "No" }.into(),
        ));
        self
    }
    pub fn choice(mut self, name: &str, choices: &[&str], default: &str) -> Flow {
        self.options.push((
            name.to_owned(),
            choices.iter().map(|c| c.to_string()).collect(),
            default.to_owned(),
        ));
        self
    }

    /// Run one step. `show` previews with the hover point as the next point; `finish`
    /// makes the result once every need is answered.
    pub fn step(
        &self,
        e: &Event,
        show: &dyn Fn(&Collected, Option<Vec3>) -> Show,
        finish: &dyn Fn(&Collected) -> Result<Reply, String>,
    ) -> Reply {
        let mut c: Collected = load(e);
        let mut error = None;
        if self.select && !c.selected {
            if e.selection.is_empty() {
                let r = Reply::ask(
                    format!("{}: select objects, then press Enter", self.label),
                    save(&c),
                )
                .want(Want::Objects);
                return if matches!(e.input, Input::Enter) {
                    r.refuse(format!("{}: nothing is selected", self.label))
                } else {
                    r
                };
            }
            c.selected = true;
            if matches!(e.input, Input::Start | Input::Selection | Input::Enter) {
                return self.ask(e, c, None, None, false, show, finish);
            }
        }
        let next = self.needs.get(c.answers.len());
        let mut hover = None;
        match &e.input {
            Input::Point { point } => match next {
                Some(Need::Point(_)) => c.answers.push(Answer::Point(*point)),
                Some(Need::Number(..)) => {
                    // A click for a number: the distance from the last point.
                    match c.points().last() {
                        Some(&last) => c.answers.push(Answer::Number(om9_addon_abi::session::dist(last, *point))),
                        None => error = Some("type a number".to_owned()),
                    }
                }
                None => {}
            },
            Input::Hover { point } => hover = Some(*point),
            Input::Undo => {
                c.answers.pop();
            }
            Input::Text { text } => match next {
                Some(Need::Point(_)) => {
                    match om9_addon_abi::session::parse_point(text, &e.plane, c.points().last().copied()) {
                        Some(p) => c.answers.push(Answer::Point(p)),
                        None => error = Some(format!("{text}: type a point (x,y or @dx,dy)")),
                    }
                }
                Some(Need::Number(..)) => match om9_addon_abi::session::parse_number(text) {
                    Some(v) => c.answers.push(Answer::Number(v)),
                    None => error = Some(format!("{text}: type a number")),
                },
                None => {}
            },
            Input::Option { name, value } => {
                if name.eq_ignore_ascii_case("Undo") {
                    c.answers.pop();
                } else if let Some(o) = self.options.iter().find(|o| o.0.eq_ignore_ascii_case(name)) {
                    let current = c.opt(self, &o.0).to_owned();
                    let v = match value {
                        Some(v) => o
                            .1
                            .iter()
                            .find(|x| x.eq_ignore_ascii_case(v))
                            .cloned()
                            .or_else(|| {
                                error = Some(format!("{}: one of {}", o.0, o.1.join(", ")));
                                None
                            }),
                        None => {
                            let i = o.1.iter().position(|x| *x == current).unwrap_or(0);
                            Some(o.1[(i + 1) % o.1.len()].clone())
                        }
                    };
                    if let Some(v) = v {
                        c.options.insert(o.0.clone(), v);
                    }
                } else {
                    error = Some(format!("{name}: unknown option"));
                }
            }
            Input::Enter => match next {
                Some(Need::Number(_, d)) => c.answers.push(Answer::Number(*d)),
                Some(Need::Point(p)) => error = Some(format!("{}: {}", self.label, p)),
                None => {}
            },
            _ => {}
        }
        let entered = matches!(e.input, Input::Enter);
        self.ask(e, c, hover, error, entered, show, finish)
    }

    #[allow(clippy::too_many_arguments)]
    fn ask(
        &self,
        e: &Event,
        c: Collected,
        hover: Option<Vec3>,
        error: Option<String>,
        entered: bool,
        show: &dyn Fn(&Collected, Option<Vec3>) -> Show,
        finish: &dyn Fn(&Collected) -> Result<Reply, String>,
    ) -> Reply {
        // With nothing to ask, Enter finishes; otherwise the last answer does.
        let complete = c.answers.len() >= self.needs.len() && (!self.needs.is_empty() || entered);
        if complete && error.is_none() {
            match finish(&c) {
                Ok(r) => return r,
                Err(why) => {
                    let mut c = c;
                    if !self.needs.is_empty() {
                        c.answers.pop();
                    }
                    return self.ask(e, c, None, Some(why), false, show, finish);
                }
            }
        }
        let (prompt, want) = match self.needs.get(c.answers.len().min(self.needs.len().saturating_sub(1))) {
            Some(Need::Point(p)) => (p.clone(), Want::Point),
            Some(Need::Number(p, d)) => (format!("{p} <{}>", om9_addon_abi::session::fmt(*d)), Want::Number),
            None => ("press Enter".to_owned(), Want::Objects),
        };
        let mut options: Vec<OptionToken> = self
            .options
            .iter()
            .map(|o| {
                let v = c.opt(self, &o.0);
                if o.1.len() == 2 && o.1[0] == "Yes" {
                    OptionToken::toggle(&o.0, v.eq_ignore_ascii_case("yes"))
                } else {
                    let choices: Vec<&str> = o.1.iter().map(String::as_str).collect();
                    OptionToken::choice(&o.0, v, &choices)
                }
            })
            .collect();
        if !c.answers.is_empty() {
            options.push(OptionToken::action("Undo"));
        }
        let shown = show(&c, hover);
        let mut r = Reply::ask(format!("{}: {prompt}", self.label), save(&c))
            .with_options(options)
            .want(want)
            .preview(shown.geo)
            .rubber(shown.rubber.or_else(|| c.points().last().copied()));
        r.preview_transforms = shown.transforms;
        match error {
            Some(why) => r.refuse(why),
            None => r,
        }
    }
}

/// `geo` with every point moved by `f`: curves (control points, or samples of circles and
/// lines refined so bends follow), surfaces (control points) and meshes with their data.
pub fn deformed(sel: &Selected, f: &dyn Fn(Vec3) -> Vec3) -> Option<Geo> {
    use om9_addon_abi::session::{add as v_add, scale as v_scale, sub as v_sub};
    let line_pts = |a: Vec3, b: Vec3| -> Vec<Vec3> {
        (0..=16)
            .map(|i| v_add(a, v_scale(v_sub(b, a), i as f64 / 16.0)))
            .map(f)
            .collect()
    };
    match &sel.exact {
        Some(Geo::Line { start, end }) => crate::interpolate(&line_pts(*start, *end), 3, false),
        Some(Geo::Nurbs {
            degree,
            points,
            weights,
            knots,
        }) => {
            // Degree-1 curves (polylines) keep their corners: refine each span.
            if *degree == 1 && points.len() < 400 {
                let mut pts = Vec::new();
                for w in points.windows(2) {
                    let seg = line_pts(w[0], w[1]);
                    pts.extend_from_slice(&seg[..seg.len() - 1]);
                }
                pts.push(f(*points.last()?));
                Some(Geo::Nurbs {
                    degree: 1,
                    knots: crate::clamped_uniform(pts.len(), 1),
                    points: pts,
                    weights: Vec::new(),
                })
            } else {
                Some(Geo::Nurbs {
                    degree: *degree,
                    points: points.iter().map(|&p| f(p)).collect(),
                    weights: weights.clone(),
                    knots: knots.clone(),
                })
            }
        }
        Some(g @ (Geo::Circle { .. } | Geo::Arc { .. })) => {
            let closed = matches!(g, Geo::Circle { .. });
            let n = 64;
            let pts: Vec<Vec3> = (0..if closed { n } else { n + 1 })
                .filter_map(|i| crate::eval_geo(g, i as f64 / n as f64))
                .map(f)
                .collect();
            crate::interpolate(&pts, 3, closed)
        }
        Some(Geo::Surface {
            degree,
            size,
            points,
            weights,
            knots,
        }) => Some(Geo::Surface {
            degree: *degree,
            size: *size,
            points: points.iter().map(|&p| f(p)).collect(),
            weights: weights.clone(),
            knots: knots.clone(),
        }),
        _ => sel.mesh.as_ref().map(|m| Geo::Mesh {
            name: sel.name.clone(),
            vertices: m.vertices.iter().map(|&p| f(p)).collect(),
            triangles: m.triangles.clone(),
            meta: None,
            layer: None,
        }),
    }
}

/// The geometry of selected objects under `f`, as replacements (objects that cannot be
/// deformed are counted).
pub fn deform_selection(
    e: &Event,
    f: &dyn Fn(Vec3) -> Vec3,
) -> (Vec<om9_addon_abi::session::Replacement>, usize) {
    let mut out = Vec::new();
    let mut skipped = 0;
    for s in &e.selection {
        match deformed(s, f) {
            Some(geo) => out.push(om9_addon_abi::session::Replacement {
                id: s.id.clone(),
                geo,
            }),
            None => skipped += 1,
        }
    }
    (out, skipped)
}

// ---- styles ----------------------------------------------------------------------------------

/// Matrix "styles": a stone remembers the builder values of the settings and cutters made on
/// it, under `styles.<command>` in its gem record, so Match Attributes and Save / Load Style
/// can make the same setting on other stones.
pub fn remember_style(e: &Event) -> Vec<om9_addon_abi::session::RecordUpdate> {
    let style: serde_json::Map<String, Value> =
        e.settings.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    e.selection
        .iter()
        .filter_map(|s| {
            let mut record = s.gem.clone()?;
            if !record.is_object() {
                return None;
            }
            if !record.get("styles").is_some_and(Value::is_object) {
                record["styles"] = json!({});
            }
            record["styles"][e.command.as_str()] = Value::Object(style.clone());
            Some(om9_addon_abi::session::RecordUpdate { id: s.id.clone(), record })
        })
        .collect()
}

/// The styles a stone remembers: command code → builder values.
pub fn styles_of(s: &Selected) -> std::collections::BTreeMap<String, Value> {
    s.gem
        .as_ref()
        .and_then(|g| g.get("styles"))
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}
