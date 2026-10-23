# OM9 add-ons

OM9 installs commands from zip packages, the way Blender installs extensions. Commands that no
add-on provides stay visible and disabled. A command an enabled add-on provides becomes
available from the same menu entry, sidebar icon, F6 search and typed CMD name (`gvGemOnCrv`).
All three resolve to one catalog code (`BuilderGemOnCrv`).

## Using add-ons

- **Tools › Add-ons…** lists installed add-ons. From there you can enable or disable an add-on,
  grant the permissions it asks for, edit its settings and uninstall it.
- **Tools › Install Add-on from Disk…**, or drag a `.zip` onto the window, installs a package.
  Install is atomic: a failed install leaves the previous version in place.
- Add-on output goes into one transaction, so one Undo removes everything a command created.
- Gems are tagged with the `om9.gem` slot, so *Super Select: Gems* finds them.
- `OM9_SAFE_MODE=1` starts OM9 without enabling any add-on.
- `OM9_CONFIG_DIR` overrides the per-version config folder. That folder holds `userprefs.toml`
  and `extensions/user/`. An `addons/` folder beside `om9.exe` is a read-only system repository.

## Package layout

```
om9_addon.toml      manifest (schema 1.0.0): id, version, type, license, om9_version_min,
                    platforms, runtime (native | data | python | wasm), entry, permissions,
                    [[commands]] (catalog code, label, icon, shortcut), [[settings]], [[assets]]
<entry>             e.g. gem_on_curve.dll for runtime = "native"
icons/…             authored SVG
```

Native add-ons export a small C ABI (`crates/om9-addon-abi`). The ABI exports these symbols:

- `om9_addon_abi_version`
- `om9_addon_command_count`
- `om9_addon_command_name`
- `om9_addon_invoke`

The host passes the selected curves, sampled in millimetres, together with the CPlane normal and
the add-on's current settings. The add-on returns meshes and messages through host callbacks.
Rust add-ons implement `sdk::Addon` and call `export_addon!`, which also catches panics at the
boundary. The host checks every returned mesh before it touches the document.

## Building packages

- `cargo run -p om9-addons --features native --bin om9-addon -- --catalog Resources/menu/MainMenu.ini build <dir> -o <pkg>.zip`
  packages a directory deterministically. `validate`, `install`, `enable`, `list`, `menu` and
  `settings` cover the rest of the lifecycle.
- On Windows CI, `tools/ci/build-addons-windows.ps1` builds `gem_on_curve.dll`, packages
  `gem_on_curve-<ver>-windows-x64.zip` and `ring_sizes-<ver>.zip`, smoke-installs them and
  uploads them as the `om9-addons` artifact. Built libraries are never committed.

## Shipped examples

| Package | Commands | Notes |
|---|---|---|
| `addons/gem_on_curve` | Gem On Crv, Gem Count On Crv | Round brilliant stones spaced along curves.<br>Closed curve: `n = ⌊L/(d+g)⌋`, `g' = L/n − d`.<br>Open curve: `n = ⌊(L+g)/(d+g)⌋`.<br>Settings: diameter, spacing, start/end %, Z offset, fit or fixed gap, plane or outward orientation. |
| `addons/ring_sizes` | — | Data library: US and ISO 8653 ring size tables. |

Check value: a Ø17.35 mm ring rail with Ø1.5 mm stones and 0.2 mm spacing gives 32 stones with a
real gap of 0.203 mm. Stones of Ø2.0 mm give 24.

Tests:

- `crates/om9-addons/tests`
- `addons/gem_on_curve/tests/native_load.rs` (real library load)
- `crates/om9-app/tests/addon_command.rs` (controller, CMD/menu paths, single Undo)
- `crates/om9-core/tests/insert_with_extension.rs`
