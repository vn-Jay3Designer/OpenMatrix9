// SPDX-License-Identifier: LGPL-2.1-or-later
//! Small helpers for writing session commands: state in and out, the stones and curves of the
//! selection, and the common "select objects → preview → Enter" flow.
use crate::gems::PlacedGem;
use crate::mesh::Frame;
use om9_addon_abi::session::{Event, Geo, Input, Reply, Selected, Vec3, Want};
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
