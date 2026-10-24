// SPDX-License-Identifier: LGPL-2.1-or-later
//! Interactive add-on commands through the real controller: start from CMD, menu code and
//! typed alias; command-line sub-options; clicked and typed points; preview without document
//! objects; one transaction on finish; Esc cancels; another command closes the session.
use om9_addon_abi::session::{Interactive, session_json};
use om9_addons::host::{Host, HostConfig, InstallOptions, Startup, build_package};
use om9_addons::manifest::RuntimeKind;
use om9_addons::runtime::{LoadRequest, Runtime};
use om9_app::addons::FeatureName;
use om9_app::session::{Controller, InvocationOrigin};
use om9_core::budget::Limits;
use om9_core::geometry::{Curve, Geometry};
use om9_core::model::Document;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Runs the real add-on code in-process through the same JSON boundary as the DLL.
struct InProcess;
impl Runtime for InProcess {
    fn load(&mut self, req: &LoadRequest) -> Result<Vec<String>, String> {
        Ok(req.manifest.commands.iter().map(|c| c.id.clone()).collect())
    }
    fn unload(&mut self, _id: &str) {}
    fn session(&mut self, id: &str, event: &[u8]) -> Result<Vec<u8>, String> {
        let (code, out) = if id == "om9_transform" {
            session_json::<om9_transform::Transforms>(event)
        } else if id == "gem_cutter" {
            session_json::<gem_cutter::GemCutter>(event)
        } else {
            session_json::<om9_curves::Curves>(event)
        };
        if code == 0 {
            Ok(out)
        } else {
            Err(String::from_utf8_lossy(&out).into())
        }
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn features() -> Vec<(String, Option<String>)> {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../assets/features.json")).unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["id"].as_str().unwrap().to_owned(),
                f["command"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

fn host(tmp: &Path) -> Arc<Mutex<Host>> {
    host_with(tmp, &["curves"])
}

fn host_with(tmp: &Path, addons: &[&str]) -> Arc<Mutex<Host>> {
    let catalog = om9_app::addons::catalog().unwrap();
    let mut h = Host::new(HostConfig::new(
        om9_app::addons::om9_version(),
        catalog.clone(),
        tmp.join("config"),
    ));
    h.register_runtime(RuntimeKind::Python, Box::new(InProcess));
    h.startup(&Startup::default());
    for name in addons {
        let zip = stage_zip(tmp, name, &catalog);
        let r = h
            .install_zip(
                &zip,
                &InstallOptions {
                    enable: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(r.enabled, "{:?}", r.warnings);
    }
    Arc::new(Mutex::new(h))
}

fn stage_zip(tmp: &Path, name: &str, catalog: &om9_addons::catalog::Catalog) -> PathBuf {
    let stage = tmp.join(format!("stage-{name}"));
    fs::create_dir_all(&stage).unwrap();
    let manifest = fs::read_to_string(repo().join(format!("addons/{name}/addon/om9_addon.toml")))
        .unwrap()
        .replace("@PLATFORM@", "any")
        .replace("runtime = \"native\"", "runtime = \"python\"")
        .replace("@ENTRY@", "entry.bin");
    fs::write(stage.join("om9_addon.toml"), manifest).unwrap();
    fs::write(stage.join("entry.bin"), b"in-process").unwrap();
    let (_, zip) = build_package(&stage, Some(catalog)).unwrap_or_else(|e| panic!("{e:?}"));
    let path = tmp.join(format!("{name}.zip"));
    fs::write(&path, zip).unwrap();
    path
}

#[test]
fn curve_commands_run_interactively() {
    assert_eq!(
        <om9_curves::Curves as Interactive>::SESSION_COMMANDS.len(),
        13
    );
    let tmp = std::env::temp_dir().join(format!("om9-app-session-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    let feats = features();
    let names: Vec<FeatureName> = feats
        .iter()
        .map(|(id, c)| FeatureName {
            id,
            command: c.as_deref(),
        })
        .collect();
    let mut c = Controller::new(Document::new("curves"), Limits::default()).unwrap();
    assert!(!c.is_available("CurveCircleCenterRadius"));
    c.attach_addons(host(&tmp), &names);
    for code in om9_curves::Curves::SESSION_COMMANDS {
        assert!(c.is_available(code), "{code} is enabled from its menu icon");
    }
    assert_eq!(
        c.addon_code("rectangle").as_deref(),
        Some("CurveRectangleCornertoCorner")
    );
    assert_eq!(
        c.addon_code("Polyline").as_deref(),
        Some("CurvePolylinePolyline")
    );

    // OM9 now has its own Polyline, Circle … : the built-in keeps the typed name and icon,
    // and the add-on version still runs by its code.
    assert!(
        c.icon_target("Polyline", None)
            .is_some_and(|t| !t.addon && t.enabled)
    );
    // Polyline: prompt + sub-options, clicks, a typed relative point, Enter.
    c.run_addon_command("CurvePolylinePolyline").unwrap();
    let s = c.addon_session().expect("session open");
    assert!(
        c.prompt_text().starts_with("Start of polyline"),
        "{}",
        c.prompt_text()
    );
    assert!(s.tokens().iter().any(|(_, t)| t == "PersistentClose=No"));
    assert!(c.session_wants_point());
    c.session_point([0.0, 0.0, 0.0]).unwrap();
    c.session_point([10.0, 0.0, 0.0]).unwrap();
    c.session_hover([10.0, 5.0, 0.0]);
    assert!(
        !c.addon_session().unwrap().hover_lines.is_empty(),
        "rubber segment preview"
    );
    assert!(
        c.document().entities.is_empty(),
        "preview is never a document object"
    );
    c.submit("@0,5").unwrap(); // relative to the last point
    c.submit("Undo").unwrap();
    c.submit("10,5").unwrap(); // absolute CPlane coordinates
    assert!(
        c.addon_session()
            .unwrap()
            .tokens()
            .iter()
            .any(|(n, _)| n == "Close")
    );
    let undo = c.coordinator().undo_len();
    c.submit("Close").unwrap();
    assert!(c.addon_session().is_none());
    assert_eq!(c.document().entities.len(), 1);
    assert_eq!(c.coordinator().undo_len(), undo + 1, "one transaction");
    let e = c.document().entities.values().next().unwrap();
    let Geometry::Curve(Curve::Nurbs(n)) = &e.geometry else {
        panic!("{:?}", e.geometry)
    };
    assert_eq!((n.degree, n.poles.len()), (1, 4), "closed: start repeated");
    assert_eq!(n.poles[2], [10.0, 5.0, 0.0]);
    assert!(c.take_addon_messages().iter().any(|m| m.contains("closed")));

    // Circle from its menu icon: Mode option, typed diameter.
    c.dispatch("CurveCircleCenterRadius", InvocationOrigin::Menu)
        .unwrap();
    c.session_point([0.0, 0.0, 0.0]).unwrap();
    c.submit("Mode=Diameter").unwrap();
    c.submit("17.35").unwrap();
    let circle = c
        .document()
        .entities
        .values()
        .find_map(|e| match &e.geometry {
            Geometry::Curve(Curve::Circle { radius, .. }) => Some(*radius),
            _ => None,
        })
        .unwrap();
    assert!((circle - 8.675).abs() < 1e-12);

    // Esc cancels; nothing is created. Another command closes the session and runs.
    let before = c.document().entities.len();
    c.run_addon_command("CurveRectangleCornertoCorner").unwrap();
    c.session_point([0.0, 0.0, 0.0]).unwrap();
    c.cancel();
    assert!(c.addon_session().is_none());
    assert_eq!(c.document().entities.len(), before);
    c.run_addon_command("CurveRectangleCornertoCorner").unwrap();
    c.run_addon_command("CurveCircleCenterRadius").unwrap();
    assert_eq!(c.addon_session().unwrap().code, "CurveCircleCenterRadius");
    c.submit("Circle").unwrap();
    assert!(
        c.addon_session().is_none(),
        "the built-in Circle closes the session"
    );
    c.cancel();
    // A refused value keeps the session open with the reason.
    c.run_addon_command("CurvePolygonCenterRadius").unwrap();
    let err = c.submit("NumSides=2").unwrap_err().to_string();
    assert!(err.contains("NumSides"), "{err}");
    assert!(c.addon_session().is_some());
    c.submit("NumSides=6").unwrap();
    c.session_point([0.0, 0.0, 0.0]).unwrap();
    c.submit("3").unwrap();
    assert!(c.addon_session().is_none());
    assert_eq!(c.document().entities.len(), before + 1);
    // Undo removes the polygon.
    c.undo().unwrap();
    assert_eq!(c.document().entities.len(), before);
    fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn transform_commands_act_on_the_selection() {
    let tmp = std::env::temp_dir().join(format!("om9-app-transform-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    let feats = features();
    let names: Vec<FeatureName> = feats
        .iter()
        .map(|(id, c)| FeatureName {
            id,
            command: c.as_deref(),
        })
        .collect();
    let mut c = Controller::new(Document::new("transform"), Limits::default()).unwrap();
    c.attach_addons(host_with(&tmp, &["curves", "transform"]), &names);
    assert!(c.is_available("OM9-TOP11-006"), "Top 11 Move icon");
    assert_eq!(c.addon_code("move").as_deref(), Some("TopIconMove"));
    // A circle to work on.
    c.run_addon_command("CurveCircleCenterRadius").unwrap();
    c.session_point([0.0, 0.0, 0.0]).unwrap();
    c.submit("2").unwrap();
    let id = *c.document().entities.keys().next().unwrap();
    let center = |c: &Controller, id| {
        let e = c.document().entities.get(&id).unwrap();
        e.placement.point([0.0; 3]).unwrap()
    };

    // Move with nothing selected asks for objects first; selecting advances the prompt.
    c.select_objects(&[]).unwrap();
    c.run_addon_command("TopIconMove").unwrap();
    assert!(
        c.prompt_text().starts_with("Select objects to move"),
        "{}",
        c.prompt_text()
    );
    c.select_objects(&[id]).unwrap();
    c.refresh_addon_session();
    assert!(
        c.prompt_text().starts_with("Point to move from"),
        "{}",
        c.prompt_text()
    );
    c.session_point([0.0, 0.0, 0.0]).unwrap();
    c.session_hover([5.0, 0.0, 0.0]);
    assert!(
        !c.addon_session().unwrap().hover_lines.is_empty(),
        "moved selection previewed"
    );
    let undo = c.coordinator().undo_len();
    c.session_point([10.0, 5.0, 0.0]).unwrap();
    assert_eq!(center(&c, id), [10.0, 5.0, 0.0]);
    assert_eq!(c.coordinator().undo_len(), undo + 1);
    assert_eq!(c.document().entities.len(), 1, "Move does not copy");

    // Polar array: 6 items → 5 copies, one transaction, one Undo.
    c.run_addon_command("TransformArrayPolar").unwrap();
    c.submit("0,0").unwrap();
    c.submit("6").unwrap();
    c.submit("").unwrap();
    assert_eq!(c.document().entities.len(), 6);
    c.undo().unwrap();
    assert_eq!(c.document().entities.len(), 1);

    // Mirror keeps the original (Copy=Yes) across the CPlane y axis.
    c.run_addon_command("TopIconMirror").unwrap();
    c.submit("YAxis").unwrap();
    assert_eq!(c.document().entities.len(), 2);
    let mirrored = c
        .document()
        .entities
        .values()
        .map(|e| e.placement.point([0.0; 3]).unwrap())
        .find(|p| p[0] < 0.0)
        .unwrap();
    assert!((mirrored[0] + 10.0).abs() < 1e-9 && (mirrored[1] - 5.0).abs() < 1e-9);

    // Scale 3D with a typed factor; Rotate with a typed angle.
    c.run_addon_command("TransformScaleScale3D").unwrap();
    c.submit("10,5").unwrap();
    c.submit("2").unwrap();
    let e = c.document().entities.get(&id).unwrap();
    assert!((e.placement.0[0][0] - 2.0).abs() < 1e-12);
    c.run_addon_command("TopIconRotate").unwrap();
    c.submit("10,5").unwrap();
    c.submit("90").unwrap();
    assert!(c.addon_session().is_none());
    fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn gem_cutter_uses_its_panel_and_command_line() {
    use om9_core::geometry::Mesh;
    use om9_core::model::{Entity, Extension, GEM_EXTENSION_SLOT};
    let tmp = std::env::temp_dir().join(format!("om9-app-cutter-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    let feats = features();
    let names: Vec<FeatureName> = feats
        .iter()
        .map(|(id, c)| FeatureName {
            id,
            command: c.as_deref(),
        })
        .collect();
    let mut doc = Document::new("cutter");
    // A Ø2 gem at (5,0,0), then moved +1 in z by its placement: the cutter must follow.
    let mut gem = Entity::new(
        "Gem 2.00 mm",
        Geometry::Mesh(Mesh {
            vertices: vec![
                [4.0, 0.0, 0.0],
                [6.0, 0.0, 0.0],
                [5.0, 1.0, 0.0],
                [5.0, 0.0, -1.0],
            ],
            faces: vec![vec![0, 1, 2], vec![0, 3, 1], vec![1, 3, 2], vec![2, 3, 0]],
            normals: vec![],
            colors: vec![],
            corner_uvs: Vec::new(),
        }),
        doc.current_layer,
    );
    gem.placement = om9_core::geometry::Transform::translation([0.0, 0.0, 1.0]);
    gem.extensions.insert(
        GEM_EXTENSION_SLOT.to_owned(),
        Extension::new(
            1,
            r#"{"cut":"round","diameter_mm":2.0,"center":[5,0,0],"axis":[0,0,1],"x":[1,0,0]}"#,
        )
        .unwrap(),
    );
    let gid = gem.id;
    doc.entities.insert(gid, gem);
    let mut c = Controller::new(doc, Limits::default()).unwrap();
    c.attach_addons(host_with(&tmp, &["gem_cutter"]), &names);
    assert_eq!(
        c.addon_code("gvAzureCutter").as_deref(),
        Some("BuilderGemCutter")
    );
    c.submit("gvAzureCutter").unwrap();
    assert!(
        c.prompt_text().starts_with("Select gems"),
        "{}",
        c.prompt_text()
    );
    assert!(
        c.addon_builder().is_some_and(|b| b.interactive),
        "panel opens with the command"
    );
    c.select_objects(&[gid]).unwrap();
    c.refresh_addon_session();
    assert!(c.prompt_text().contains("1 cutters"), "{}", c.prompt_text());
    assert!(
        !c.addon_session().unwrap().lines.is_empty(),
        "cutter preview"
    );
    // Command-line option sets the panel value; the preview follows.
    c.submit("GirdleScale=120").unwrap();
    assert_eq!(c.addon_builder().unwrap().tokens()[0].1, "GirdleScale=120");
    let before = c.document().entities.len();
    c.submit("").unwrap();
    assert!(
        c.addon_session().is_none() && c.addon_builder().is_none(),
        "{} / {:?}",
        c.prompt_text(),
        c.take_addon_messages()
    );
    assert_eq!(c.document().entities.len(), before + 1);
    let cutter = c
        .document()
        .entities
        .values()
        .find(|e| e.label.starts_with("Gem Cutter"))
        .unwrap();
    assert!(
        cutter.extension(GEM_EXTENSION_SLOT).is_none(),
        "a cutter is not a gem"
    );
    let Geometry::Mesh(m) = &cutter.geometry else {
        panic!()
    };
    let top = m.vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
    assert!(
        (top - 2.0).abs() < 1e-9,
        "top 50 % of Ø2 above the moved girdle at z=1: {top}"
    );
    let widest = m
        .vertices
        .iter()
        .map(|v| ((v[0] - 5.0).powi(2) + v[1] * v[1]).sqrt())
        .fold(0.0, f64::max);
    assert!(
        (widest - 1.2).abs() < 1e-9,
        "GirdleScale 120 % of Ø2: {widest}"
    );
    // The values accepted become the panel defaults next time.
    c.submit("gvAzureCutter").unwrap();
    assert_eq!(c.addon_builder().unwrap().tokens()[0].1, "GirdleScale=120");
    c.cancel();
    fs::remove_dir_all(&tmp).unwrap();
}

/// Every command the bundled add-ons provide has an OM9 icon whose catalog code calls it
/// once the add-on is installed (menu palettes, toolbars and icon history share this lookup).
#[test]
fn every_addon_command_has_an_icon_that_calls_it() {
    let tmp = std::env::temp_dir().join(format!("om9-app-icons-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    let feats = features();
    let names: Vec<FeatureName> = feats
        .iter()
        .map(|(id, c)| FeatureName {
            id,
            command: c.as_deref(),
        })
        .collect();
    let addons = ["curves", "transform", "gem_cutter"];
    let mut c = Controller::new(Document::new("icons"), Limits::default()).unwrap();
    c.attach_addons(host_with(&tmp, &addons), &names);
    for name in addons {
        let text =
            fs::read_to_string(repo().join(format!("addons/{name}/addon/om9_addon.toml"))).unwrap();
        let codes: Vec<&str> = text
            .split("[[commands]]")
            .skip(1)
            .filter_map(|block| block.lines().find_map(|l| l.trim().strip_prefix("id = ")))
            .map(|v| v.trim_matches('"'))
            .collect();
        assert!(!codes.is_empty(), "{name}");
        for code in codes {
            let icon = om9_app::icons::icon_for_command(code)
                .unwrap_or_else(|| panic!("{name}: no OM9 icon for {code}"));
            let target = c
                .icon_target(icon, None)
                .unwrap_or_else(|| panic!("{icon}: no command"));
            assert!(target.enabled, "{icon} → {target:?}");
            // Either OM9's own command (it keeps its icon) or this add-on's.
            assert!(
                !target.addon || target.command == code,
                "{icon} → {target:?}, want {code}"
            );
            assert!(c.is_available(code), "{code}");
        }
    }
    fs::remove_dir_all(&tmp).unwrap();
}
