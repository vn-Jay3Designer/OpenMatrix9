// SPDX-License-Identifier: LGPL-2.1-or-later
//! Running commands provided by add-ons (Preferences › Add-ons).
//!
//! The add-on receives copies only: the selected curves sampled to polylines in world
//! millimetres, the active CPlane normal and its validated settings. It returns meshes
//! through the host, which are checked again and inserted in **one transaction**, so one
//! Undo removes everything the command created. Gems are tagged with the `om9.gem`
//! extension so Tools › Super Select: Gems finds them.
use super::Controller;
use crate::addons::{FeatureName, SharedHost};
use om9_addons::runtime::{CommandInput, CurveInput};
use om9_core::{
    Error, Result,
    geometry::{Geometry, Mesh},
    model::{Entity, Extension, GEM_EXTENSION_SLOT},
    transaction::{Mutation, TransactionRequest},
};

/// Samples per selected curve. 1025 = 2^10 + 1 keeps polycurve seams exact for
/// power-of-two segment counts; the tangent error at this density is far below 0.01°.
pub const CURVE_SAMPLES: usize = 1025;

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

impl Controller {
    /// Attach the add-on host and the feature rows used to resolve typed command names.
    pub fn attach_addons(&mut self, host: SharedHost, features: &[FeatureName<'_>]) {
        self.addons = Some(host);
        self.refresh_addon_names(features);
    }
    /// Carry the add-on host and its name table onto a replacement controller
    /// (File › New / Open / Revert). Messages not yet shown move with them.
    pub(super) fn adopt_addons(&mut self, previous: &mut Self) {
        if self.addons.is_none() {
            self.addons = previous.addons.take();
            self.addon_names = std::mem::take(&mut previous.addon_names);
        }
        self.addon_messages
            .append(&mut std::mem::take(&mut previous.addon_messages));
    }
    pub fn addon_host(&self) -> Option<&SharedHost> {
        self.addons.as_ref()
    }
    /// Rebuild the command-name table after an add-on was installed, enabled or disabled.
    pub fn refresh_addon_names(&mut self, features: &[FeatureName<'_>]) {
        self.addon_names = match &self.addons {
            Some(host) => match host.lock() {
                Ok(host) => crate::addons::provided_names(&host, features),
                Err(_) => Default::default(),
            },
            None => Default::default(),
        };
    }
    /// The catalog code an add-on provides for a typed name, feature id or code.
    pub fn addon_code(&self, name: &str) -> Option<String> {
        let key = name.trim().to_ascii_lowercase();
        (!key.is_empty())
            .then(|| self.addon_names.get(&key).cloned())
            .flatten()
    }
    /// Menu/icon/F6 gate: a built-in operation or a command an enabled add-on provides.
    pub fn is_available(&self, feature: &str) -> bool {
        Self::availability(feature).is_ok() || self.addon_code(feature).is_some()
    }
    pub fn take_addon_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.addon_messages)
    }

    /// Run an add-on command against the current selection.
    pub(super) fn run_addon_command(&mut self, code: &str) -> Result<()> {
        if self.is_running() {
            return Err(Error::Busy);
        }
        let host = self
            .addons
            .clone()
            .ok_or(Error::Unsupported("add-on host"))?;
        let to_mm = self.document().page_unit.factor();
        let limits = *self.coordinator.limits();
        let mut curves = Vec::new();
        for id in &self.selection {
            let Some(entity) = self.document().entities.get(id) else {
                continue;
            };
            let Geometry::Curve(curve) = &entity.geometry else {
                continue;
            };
            let samples = om9_core::surface_kernel::sample_curve(curve, CURVE_SAMPLES, &limits)?;
            let mut points = Vec::with_capacity(samples.len());
            for p in samples {
                let q = entity.placement.point(p)?;
                points.push([q[0] * to_mm, q[1] * to_mm, q[2] * to_mm]);
            }
            let closed = points.len() > 2 && {
                let (a, b) = (points[0], points[points.len() - 1]);
                let span = (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2]);
                span <= 1e-6
            };
            if closed {
                points.pop();
            }
            curves.push(CurveInput { points, closed });
        }
        let plane = self.workspace.views[&self.workspace.active].cplane;
        let input = CommandInput {
            command: code.to_owned(),
            curves,
            plane_normal: cross(plane.x, plane.y),
            settings: Default::default(),
        };
        self.events.emit(
            crate::command_events::EventKind::Started,
            code,
            "add-on command",
            None,
        );
        let output = host
            .lock()
            .map_err(|_| Error::Io("add-on host poisoned".into()))?
            .invoke(code, input);
        let output = match output {
            Ok(output) => output,
            Err(message) => {
                self.events.emit(
                    crate::command_events::EventKind::Finished,
                    code,
                    "add-on refused",
                    Some(crate::command_events::EventOutcome::Refused),
                );
                return Err(Error::Io(message));
            }
        };
        self.addon_messages.extend(output.messages.iter().cloned());
        if output.meshes.is_empty() {
            self.events.emit(
                crate::command_events::EventKind::Finished,
                code,
                "reported",
                None,
            );
            return Ok(());
        }
        let layer = self.document().current_layer;
        let mut mutations = Vec::with_capacity(output.meshes.len());
        for m in output.meshes {
            let mesh = Mesh {
                vertices: m
                    .vertices
                    .iter()
                    .map(|v| [v[0] / to_mm, v[1] / to_mm, v[2] / to_mm])
                    .collect(),
                faces: m.triangles.iter().map(|t| t.to_vec()).collect(),
                normals: Vec::new(),
                colors: Vec::new(),
            };
            let mut entity = Entity::new(&m.name, Geometry::Mesh(mesh), layer);
            if entity.attributes.layer.is_some() {
                entity.attributes.color = None;
            }
            if let Ok(extension) = Extension::new(1, m.meta_json.clone()) {
                entity
                    .extensions
                    .insert(GEM_EXTENSION_SLOT.to_owned(), extension);
            }
            mutations.push(Mutation::Insert {
                document: self.document,
                entity,
            });
        }
        if self.scene.is_none() {
            return Err(Error::Missing("command scene"));
        }
        let created = mutations.len();
        let held = self.scene_cache_usage()?;
        let label = crate::icons::icon_name(code).unwrap_or(code).to_owned();
        let prepared = self
            .coordinator
            .prepare_with_held_usage(TransactionRequest::new(&label, mutations), held)?;
        let receipt = self.coordinator.commit(prepared)?;
        if receipt.published {
            self.refresh_scene()?;
            self.sync_published(om9_core::changes::ChangeCause::Edit);
        }
        self.events.emit(
            crate::command_events::EventKind::Finished,
            code,
            &format!("created {created} objects"),
            Some(if receipt.published {
                crate::command_events::EventOutcome::Committed
            } else {
                crate::command_events::EventOutcome::Refused
            }),
        );
        Ok(())
    }
}
