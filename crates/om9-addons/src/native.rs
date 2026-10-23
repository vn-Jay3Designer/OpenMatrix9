// SPDX-License-Identifier: LGPL-2.1-or-later
//! Runtime add-on native: nạp DLL/.so theo `om9-addon-abi`.
//!
//! Ranh giới an toàn: mọi dữ liệu add-on trả về đi qua callback, được sao chép và kiểm tra
//! (độ dài, chỉ số, số hữu hạn) trước khi tới host. Thư viện chỉ được giải phóng khi add-on tắt,
//! sau khi mọi lời gọi đã trả về. Add-on native chạy cùng tiến trình nên không phải sandbox:
//! chỉ dùng cho add-on tin cậy (mặc định của OM9 cho kho system/user).

use crate::runtime::{
    CommandInput, CommandOutput, LoadRequest, MAX_MESSAGES, MAX_OUTPUT_MESHES, MAX_OUTPUT_VERTICES,
    MeshOutput, Runtime,
};
use crate::settings::SettingValue;
use om9_addon_abi::*;
use std::collections::BTreeMap;
use std::ffi::c_void;

struct Loaded {
    // Giữ thư viện sống; con trỏ hàm bên dưới chỉ hợp lệ khi `_lib` còn.
    _lib: libloading::Library,
    invoke: InvokeFn,
}

#[derive(Default)]
pub struct NativeRuntime {
    loaded: BTreeMap<String, Loaded>,
}

impl NativeRuntime {
    pub fn new() -> NativeRuntime {
        NativeRuntime::default()
    }
}

#[derive(Default)]
struct Collector {
    meshes: Vec<MeshOutput>,
    messages: Vec<String>,
    vertices: usize,
    error: Option<String>,
}

unsafe extern "C" fn cb_message(host: *mut c_void, text: Om9Str) {
    if host.is_null() {
        return;
    }
    // SAFETY: `host` là `&mut Collector` do `invoke` cấp, chỉ dùng trong lời gọi.
    let c = unsafe { &mut *(host as *mut Collector) };
    if c.messages.len() >= MAX_MESSAGES {
        return;
    }
    // SAFETY: chuỗi do add-on cấp, còn sống suốt callback.
    let t = unsafe { text.to_str() }.unwrap_or("(thông điệp không phải UTF-8)");
    c.messages.push(t.chars().take(2000).collect());
}

unsafe extern "C" fn cb_add_mesh(
    host: *mut c_void,
    name: Om9Str,
    vertices: *const [f64; 3],
    vertex_count: usize,
    triangles: *const [u32; 3],
    triangle_count: usize,
    meta: Om9Str,
) -> i32 {
    if host.is_null() {
        return -1;
    }
    // SAFETY: như `cb_message`.
    let c = unsafe { &mut *(host as *mut Collector) };
    let fail = |c: &mut Collector, e: String| {
        c.error.get_or_insert(e);
        -1
    };
    if c.meshes.len() >= MAX_OUTPUT_MESHES {
        return fail(c, format!("quá {MAX_OUTPUT_MESHES} mesh"));
    }
    if vertex_count < 3 || triangle_count == 0 || vertices.is_null() || triangles.is_null() {
        return fail(c, "mesh rỗng".into());
    }
    if vertex_count > MAX_OUTPUT_VERTICES - c.vertices || triangle_count > 4 * MAX_OUTPUT_VERTICES {
        return fail(c, format!("quá {MAX_OUTPUT_VERTICES} đỉnh"));
    }
    // SAFETY: add-on cấp mảng đúng độ dài, còn sống suốt callback; ta sao chép ngay.
    let v = unsafe { std::slice::from_raw_parts(vertices, vertex_count) }.to_vec();
    let t = unsafe { std::slice::from_raw_parts(triangles, triangle_count) }.to_vec();
    let name = unsafe { name.to_str() }
        .unwrap_or("Gem")
        .chars()
        .take(120)
        .collect::<String>();
    let meta_json = unsafe { meta.to_str() }.unwrap_or("").to_owned();
    if v.iter().flatten().any(|x| !x.is_finite()) {
        return fail(c, format!("mesh {name:?} có toạ độ không hữu hạn"));
    }
    if t.iter().flatten().any(|&i| i as usize >= vertex_count) {
        return fail(c, format!("mesh {name:?} có chỉ số đỉnh vượt phạm vi"));
    }
    c.vertices += vertex_count;
    c.meshes.push(MeshOutput {
        name,
        vertices: v,
        triangles: t,
        meta_json,
    });
    0
}

impl Runtime for NativeRuntime {
    fn load(&mut self, req: &LoadRequest) -> Result<Vec<String>, String> {
        let entry = req
            .manifest
            .entry
            .as_deref()
            .ok_or("manifest thiếu entry")?;
        let path = req.dir.join(entry);
        if !path.is_file() {
            return Err(format!("không tìm thấy thư viện {entry}"));
        }
        // SAFETY: nạp thư viện native chạy hàm khởi tạo của nó; đây là ranh giới tin cậy của add-on
        // native (đã được người dùng cài và bật).
        let lib = unsafe { libloading::Library::new(&path) }
            .map_err(|e| format!("không nạp được {entry}: {e}"))?;
        // SAFETY: kiểu ký hiệu theo om9-addon-abi; sai kiểu sẽ bị chặn bởi kiểm tra ABI ngay sau.
        let (abi, count, name, invoke) = unsafe {
            let abi = *lib.get::<AbiVersionFn>(SYM_ABI_VERSION).map_err(|_| {
                format!("{entry} không phải add-on OM9 (thiếu om9_addon_abi_version)")
            })?;
            let count = *lib
                .get::<CommandCountFn>(SYM_COMMAND_COUNT)
                .map_err(|_| "thiếu om9_addon_command_count".to_string())?;
            let name = *lib
                .get::<CommandNameFn>(SYM_COMMAND_NAME)
                .map_err(|_| "thiếu om9_addon_command_name".to_string())?;
            let invoke = *lib
                .get::<InvokeFn>(SYM_INVOKE)
                .map_err(|_| "thiếu om9_addon_invoke".to_string())?;
            (abi, count, name, invoke)
        };
        // SAFETY: các hàm không nhận con trỏ từ host.
        let version = unsafe { abi() };
        if version != ABI_VERSION {
            return Err(format!(
                "{entry} dùng ABI {version}, OM9 này hỗ trợ ABI {ABI_VERSION}"
            ));
        }
        let n = unsafe { count() };
        if n > 4096 {
            return Err("add-on khai báo quá nhiều lệnh".into());
        }
        let mut commands = Vec::with_capacity(n);
        for i in 0..n {
            // SAFETY: add-on trả chuỗi tĩnh của chính nó; ta sao chép khi thư viện còn nạp.
            let s = unsafe { name(i).to_str() }.ok_or("tên lệnh không phải UTF-8")?;
            commands.push(s.to_owned());
        }
        self.loaded
            .insert(req.id.to_owned(), Loaded { _lib: lib, invoke });
        Ok(commands)
    }

    fn unload(&mut self, id: &str) {
        self.loaded.remove(id);
    }

    fn invoke(&mut self, id: &str, input: &CommandInput) -> Result<CommandOutput, String> {
        let loaded = self
            .loaded
            .get(id)
            .ok_or_else(|| format!("add-on {id} chưa nạp"))?;
        let curves: Vec<Om9Curve> = input
            .curves
            .iter()
            .map(|c| Om9Curve {
                points: c.points.as_ptr(),
                count: c.points.len(),
                closed: c.closed as u8,
            })
            .collect();
        let texts: Vec<(String, Option<String>)> = input
            .settings
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    if let SettingValue::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    },
                )
            })
            .collect();
        let settings: Vec<Om9Setting> = input
            .settings
            .values()
            .zip(&texts)
            .map(|(v, (k, t))| {
                let (kind, number) = match v {
                    SettingValue::Bool(b) => (SETTING_BOOL, *b as u8 as f64),
                    SettingValue::Int(i) => (SETTING_NUMBER, *i as f64),
                    SettingValue::Float(f) => (SETTING_NUMBER, *f),
                    SettingValue::Str(_) => (SETTING_TEXT, 0.0),
                };
                Om9Setting {
                    key: Om9Str::new(k),
                    kind,
                    number,
                    text: t.as_deref().map(Om9Str::new).unwrap_or(Om9Str::EMPTY),
                }
            })
            .collect();
        let mut collector = Collector::default();
        let inv = Om9Invocation {
            abi_version: ABI_VERSION,
            command: Om9Str::new(&input.command),
            curves: curves.as_ptr(),
            curve_count: curves.len(),
            settings: settings.as_ptr(),
            setting_count: settings.len(),
            plane_normal: input.plane_normal,
            host: Om9HostApi {
                host: &mut collector as *mut Collector as *mut c_void,
                add_mesh: cb_add_mesh,
                message: cb_message,
            },
        };
        // SAFETY: mọi con trỏ trong `inv` trỏ vào dữ liệu sống tới hết lời gọi; add-on không được giữ lại.
        let code = unsafe { (loaded.invoke)(&inv) };
        if let Some(e) = collector.error.take() {
            return Err(format!("kết quả add-on bị từ chối: {e}"));
        }
        let out = CommandOutput {
            meshes: collector.meshes,
            messages: collector.messages,
        };
        match code {
            INVOKE_OK => {
                out.validate()?;
                Ok(out)
            }
            _ => {
                let why = out
                    .messages
                    .last()
                    .cloned()
                    .unwrap_or_else(|| format!("mã lỗi {code}"));
                Err(why)
            }
        }
    }
}
