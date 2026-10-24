# OM9 add-ons

OM9 installs commands from zip packages, the way Blender installs extensions. Commands that no
add-on provides stay visible and disabled. A command an enabled add-on provides becomes
available from the same menu entry, sidebar icon, F6 search and typed CMD name (`gvGemOnCrv`).
All three resolve to one catalog code (`BuilderGemOnCrv`).

## Using add-ons

- **Tools › Add-ons…** lists installed add-ons. From there you can enable or disable an add-on,
  grant the permissions it asks for, start its commands and uninstall it. Parameters are not
  edited there: they are set while the command runs (see *Builders* below).
- **Tools › Install Add-on from Disk…**, or drag a `.zip` onto the window, installs a package.
  Install is atomic: a failed install leaves the previous version in place.
- Add-on output goes into one transaction, so one Undo removes everything a command created.
- Gems are tagged with the `om9.gem` slot, so *Super Select: Gems* finds them.
- `OM9_SAFE_MODE=1` starts OM9 without enabling any add-on.
- `OM9_CONFIG_DIR` overrides the per-version config folder. That folder holds `userprefs.toml`
  and `extensions/user/`. An `addons/` folder beside `om9.exe` is a read-only system repository.

## Builders: parameters while the command runs

A command declared with `builder = true` does not create anything when it starts. It opens a
builder instead:

- The **command line** shows the prompt and the options as tokens, for example
  `Size=1.5  Spacing=0.2  Start=0  End=100  ZOffset=0  Layout=Dàn đều  Orient=Theo CPlane`.
  - Type `Size=2` to change a value. Option names match case-insensitively or by a unique
    prefix (`spa=0,25`).
  - A choice or flag steps to its next value when you type its bare name (`Layout`) or click it.
  - Clicking a number token puts `Size=` in the line.
- The **BUILDERS** panel below the Project Manager shows the command thumbnail (its menu icon
  and a sketch of the current diameter and gap), the live result, Accept / Cancel / Defaults,
  every parameter as a draggable or typable field, and one slider for the selected parameter.
- The **viewports** show the result as a cyan outline for the selected curves. The outline is
  not a document object. You can pick or change the curve while the builder is open.
- **Enter** or **✔ Đặt** creates the result in one transaction (one Undo). The accepted values
  become the add-on's defaults for next time.
- **Esc** or **✖ Huỷ** closes the builder; nothing is created or stored. Starting another
  command also closes it.

Manifest keys used by builders:

- `[[commands]] builder = true`.
- `[[settings]] option = "Size"`: the command-line name. It defaults to the key in PascalCase.
- `[[settings]] choice_labels = [...]`: display text for enum choices.

Commands without `builder = true` (for example Gem Count On Crv) run at once with the
remembered values.

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
| `addons/gem_on_curve` | Gem On Crv, Gem Count On Crv | Round brilliant stones spaced along curves.<br>Closed curve: `n = ⌊L/(d+g)⌋`, `g' = L/n − d`.<br>Open curve: `n = ⌊(L+g)/(d+g)⌋`.<br>Gem On Crv opens a builder; Gem Count On Crv reports at once.<br>Options: Size, Spacing, Start/End %, ZOffset, Layout (fit or fixed gap), Orient (CPlane or outward). |
| `addons/ring_sizes` | — | Data library: US and ISO 8653 ring size tables. |

Check value: a Ø17.35 mm ring rail with Ø1.5 mm stones and 0.2 mm spacing gives 32 stones with a
real gap of 0.203 mm. Stones of Ø2.0 mm give 24.

Tests:

- `crates/om9-addons/tests`
- `addons/gem_on_curve/tests/native_load.rs` (real library load)
- `crates/om9-app/tests/addon_command.rs` (controller, CMD/menu paths, single Undo)
- `crates/om9-core/tests/insert_with_extension.rs`
