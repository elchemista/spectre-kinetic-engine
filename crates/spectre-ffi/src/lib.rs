use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;

pub struct FfiHandle {
    dispatcher: spectre_core::SpectreDispatcher,
}

#[cfg(feature = "rustler")]
mod rustler;

/// # Safety
///
/// - `handle` must be a valid pointer returned by `spectre_open`.
/// - `al_text` must be a valid, NUL-terminated UTF-8 string containing the AL statement.
/// - `out_plan_json` and `err_out` may be null; when non-null, on success/failure they will be set
///   to newly allocated C strings which must be freed by the caller with `spectre_free_string`.
#[no_mangle]
pub unsafe extern "C" fn spectre_plan_al(
    handle: *mut FfiHandle,
    al_text: *const c_char,
    out_plan_json: *mut *mut c_char,
    err_out: *mut *mut c_char,
) -> i32 {
    if handle.is_null() || al_text.is_null() {
        set_err(err_out, "null argument");
        return 1;
    }
    let handle = &*handle;

    let al = match CStr::from_ptr(al_text).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_err(err_out, "invalid UTF-8 for al_text");
            return 2;
        }
    };

    let res = (|| -> anyhow::Result<String> {
        let plan = handle.dispatcher.plan_al(al, None, None, None);
        let out = serde_json::to_string(&plan)?;
        Ok(out)
    })();

    match res {
        Ok(json) => {
            if !out_plan_json.is_null() {
                let c = CString::new(json).unwrap_or_else(|_| CString::new("{}").unwrap());
                *out_plan_json = c.into_raw();
            }
            0
        }
        Err(e) => {
            set_err(err_out, &format!("{}", e));
            3
        }
    }
}

/// # Safety
///
/// - `handle` must be a valid pointer returned by `spectre_open`.
/// - `action_json` must be a valid, NUL-terminated UTF-8 string containing a `ToolDef` JSON object.
/// - `err_out` may be null; when non-null, on failure it will be set to a newly allocated C string
///   that must be freed by the caller with `spectre_free_string`.
#[no_mangle]
pub unsafe extern "C" fn spectre_add_action(
    handle: *mut FfiHandle,
    action_json: *const c_char,
    err_out: *mut *mut c_char,
) -> i32 {
    if handle.is_null() || action_json.is_null() {
        set_err(err_out, "null argument");
        return 1;
    }
    let handle = &mut *handle;

    let action_str = match CStr::from_ptr(action_json).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_err(err_out, "invalid UTF-8 for action_json");
            return 2;
        }
    };

    let res = (|| -> anyhow::Result<()> {
        let action: spectre_core::types::ToolDef = serde_json::from_str(action_str)?;
        handle.dispatcher.add_action(action)?;
        Ok(())
    })();

    match res {
        Ok(()) => 0,
        Err(e) => {
            set_err(err_out, &format!("{}", e));
            3
        }
    }
}

/// # Safety
///
/// - `handle` must be a valid pointer returned by `spectre_open`.
/// - `action_id` must be a valid, NUL-terminated UTF-8 string.
/// - `out_deleted` may be null; when non-null it is set to 1 if removed, 0 if not found.
/// - `err_out` may be null; when non-null, on failure it will be set to a newly allocated C string
///   that must be freed by the caller with `spectre_free_string`.
#[no_mangle]
pub unsafe extern "C" fn spectre_delete_action(
    handle: *mut FfiHandle,
    action_id: *const c_char,
    out_deleted: *mut i32,
    err_out: *mut *mut c_char,
) -> i32 {
    if handle.is_null() || action_id.is_null() {
        set_err(err_out, "null argument");
        return 1;
    }
    let handle = &mut *handle;

    let action_id = match CStr::from_ptr(action_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_err(err_out, "invalid UTF-8 for action_id");
            return 2;
        }
    };

    match handle.dispatcher.delete_action(action_id) {
        Ok(deleted) => {
            if !out_deleted.is_null() {
                *out_deleted = if deleted { 1 } else { 0 };
            }
            0
        }
        Err(e) => {
            set_err(err_out, &format!("{}", e));
            3
        }
    }
}

/// # Safety
///
/// - `handle` must be a valid pointer returned by `spectre_open`.
/// - `registry_mcr` must be a valid, NUL-terminated UTF-8 path to a compiled `.mcr` file.
/// - `err_out` may be null; when non-null, on failure it will be set to a newly allocated C string
///   that must be freed by the caller with `spectre_free_string`.
#[no_mangle]
pub unsafe extern "C" fn spectre_load_registry(
    handle: *mut FfiHandle,
    registry_mcr: *const c_char,
    err_out: *mut *mut c_char,
) -> i32 {
    if handle.is_null() || registry_mcr.is_null() {
        set_err(err_out, "null argument");
        return 1;
    }
    let handle = &mut *handle;

    let registry_mcr = match CStr::from_ptr(registry_mcr).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_err(err_out, "invalid UTF-8 for registry_mcr");
            return 2;
        }
    };

    match handle.dispatcher.set_registry(Path::new(registry_mcr)) {
        Ok(()) => 0,
        Err(e) => {
            set_err(err_out, &format!("{}", e));
            3
        }
    }
}

/// # Safety
///
/// AL-only convenience alias for `spectre_plan_al`.
///
/// - `handle` must be a valid pointer returned by `spectre_open`.
/// - `al_text` must be a valid, NUL-terminated UTF-8 string containing the AL statement.
/// - `out_plan_json` and `err_out` follow the same ownership rules as `spectre_plan_al`.
#[no_mangle]
pub unsafe extern "C" fn spectre_plan(
    handle: *mut FfiHandle,
    al_text: *const c_char,
    out_plan_json: *mut *mut c_char,
    err_out: *mut *mut c_char,
) -> i32 {
    spectre_plan_al(handle, al_text, out_plan_json, err_out)
}

/// # Safety
///
/// - `model_dir` and `registry_mcr` must be valid, NUL-terminated UTF-8 strings.
/// - `err_out` may be null; when non-null, on error it will be set to a newly allocated C string
///   that must be freed by the caller with `spectre_free_string`.
/// - Returns a non-null opaque pointer on success. The handle must later be released with `spectre_close`.
#[no_mangle]
pub unsafe extern "C" fn spectre_open(
    model_dir: *const c_char,
    registry_mcr: *const c_char,
    err_out: *mut *mut c_char,
) -> *mut FfiHandle {
    if model_dir.is_null() || registry_mcr.is_null() {
        set_err(err_out, "null argument");
        return std::ptr::null_mut();
    }

    let model_dir = match CStr::from_ptr(model_dir).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_err(err_out, "invalid UTF-8 for model_dir");
            return std::ptr::null_mut();
        }
    };
    let registry_mcr = match CStr::from_ptr(registry_mcr).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_err(err_out, "invalid UTF-8 for registry_mcr");
            return std::ptr::null_mut();
        }
    };

    let res = (|| -> anyhow::Result<FfiHandle> {
        let (_meta, embedder) = spectre_core::pack::load_pack(Path::new(model_dir))?;
        let compiled = spectre_core::CompiledRegistry::load(Path::new(registry_mcr))?;
        let dispatcher = spectre_core::SpectreDispatcher::new(embedder, compiled);
        Ok(FfiHandle { dispatcher })
    })();

    match res {
        Ok(handle) => Box::into_raw(Box::new(handle)),
        Err(e) => {
            set_err(err_out, &format!("{}", e));
            std::ptr::null_mut()
        }
    }
}

/// # Safety
///
/// - `handle` must be a pointer previously returned by `spectre_open` or null.
/// - After this call, `handle` must not be used again.
#[no_mangle]
pub unsafe extern "C" fn spectre_close(handle: *mut FfiHandle) {
    if handle.is_null() {
        return;
    }
    let _boxed: Box<FfiHandle> = Box::from_raw(handle);
}

/// # Safety
///
/// - `handle` must be a valid pointer returned by `spectre_open`.
/// - `request_json` must be a valid, NUL-terminated UTF-8 string containing a `PlanRequest` JSON.
/// - `out_plan_json` and `err_out` may be null; when non-null, on success/failure they will be set
///   to newly allocated C strings which must be freed by the caller with `spectre_free_string`.
#[no_mangle]
pub unsafe extern "C" fn spectre_plan_json(
    handle: *mut FfiHandle,
    request_json: *const c_char,
    out_plan_json: *mut *mut c_char,
    err_out: *mut *mut c_char,
) -> i32 {
    if handle.is_null() || request_json.is_null() {
        set_err(err_out, "null argument");
        return 1;
    }
    let handle = &*handle;

    let req_str = match CStr::from_ptr(request_json).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_err(err_out, "invalid UTF-8 for request_json");
            return 2;
        }
    };

    let res = (|| -> anyhow::Result<String> {
        let request: spectre_core::PlanRequest = serde_json::from_str(req_str)?;
        let plan = handle.dispatcher.plan(&request);
        let out = serde_json::to_string(&plan)?;
        Ok(out)
    })();

    match res {
        Ok(json) => {
            if !out_plan_json.is_null() {
                let c = CString::new(json).unwrap_or_else(|_| CString::new("{}").unwrap());
                *out_plan_json = c.into_raw();
            }
            0
        }
        Err(e) => {
            set_err(err_out, &format!("{}", e));
            3
        }
    }
}

/// # Safety
///
/// - `ptr` must be a pointer previously returned by this library (e.g., `spectre_version`,
///   `spectre_plan_json` via its output pointer), or null.
/// - Passing any other pointer is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn spectre_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

#[no_mangle]
pub extern "C" fn spectre_version() -> *mut c_char {
    CString::new(env!("CARGO_PKG_VERSION")).unwrap().into_raw()
}

unsafe fn set_err(err_out: *mut *mut c_char, msg: &str) {
    if !err_out.is_null() {
        let c = CString::new(msg).unwrap_or_else(|_| CString::new("error").unwrap());
        *err_out = c.into_raw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectre_core::types::{ToolDef, ToolMeta};
    use std::path::{Path, PathBuf};

    fn fixture_paths() -> (PathBuf, PathBuf) {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let model = manifest.join("../../packs/minilm");
        let registry = manifest.join("../spectre-core/tests/test_registry.mcr");
        (model, registry)
    }

    fn dynamic_action_json() -> String {
        serde_json::to_string(&ToolDef {
            id: "Dynamic.Echo.say/1".to_string(),
            module: "Dynamic.Echo".to_string(),
            name: "say".to_string(),
            arity: 1,
            doc: "Echo a user message".to_string(),
            spec: "say(message :: String.t()) :: :ok".to_string(),
            args: vec![spectre_core::types::ArgDef {
                name: "message".to_string(),
                arg_type: "String.t()".to_string(),
                required: true,
                aliases: vec!["text".to_string(), "msg".to_string()],
                default: None,
            }],
            examples: vec!["DYNAMIC ECHO SAY WITH: MESSAGE={message}".to_string()],
        })
        .expect("serialize action")
    }

    #[test]
    fn ffi_add_and_delete_action_should_work() {
        let (model, registry) = fixture_paths();
        let model_c = CString::new(model.to_string_lossy().into_owned()).unwrap();
        let registry_c = CString::new(registry.to_string_lossy().into_owned()).unwrap();

        let mut err: *mut c_char = std::ptr::null_mut();
        let handle = unsafe { spectre_open(model_c.as_ptr(), registry_c.as_ptr(), &mut err) };
        assert!(!handle.is_null(), "spectre_open failed");

        let action_json = CString::new(dynamic_action_json()).unwrap();
        let rc_add = unsafe { spectre_add_action(handle, action_json.as_ptr(), &mut err) };
        assert_eq!(rc_add, 0, "spectre_add_action failed");

        let action_id = CString::new("Dynamic.Echo.say/1").unwrap();
        let mut deleted: i32 = 0;
        let rc_del = unsafe { spectre_delete_action(handle, action_id.as_ptr(), &mut deleted, &mut err) };
        assert_eq!(rc_del, 0, "spectre_delete_action failed");
        assert_eq!(deleted, 1, "expected deleted=1");

        unsafe {
            spectre_close(handle);
            if !err.is_null() {
                spectre_free_string(err);
            }
        }
    }

    #[test]
    fn ffi_load_registry_dimension_mismatch_should_fail() {
        let (model, registry) = fixture_paths();
        let model_c = CString::new(model.to_string_lossy().into_owned()).unwrap();
        let registry_c = CString::new(registry.to_string_lossy().into_owned()).unwrap();

        let mut err: *mut c_char = std::ptr::null_mut();
        let handle = unsafe { spectre_open(model_c.as_ptr(), registry_c.as_ptr(), &mut err) };
        assert!(!handle.is_null(), "spectre_open failed");

        let bad_registry = spectre_core::CompiledRegistry {
            tools: vec![ToolMeta {
                id: "Bad.Mod.fn/0".to_string(),
                module: "Bad.Mod".to_string(),
                name: "fn".to_string(),
                arity: 0,
                args: Vec::new(),
                param_range: (0, 0),
            }],
            dims: 1,
            tokenizer_hash: "bad".to_string(),
            tool_embeddings: ndarray::Array2::zeros((1, 1)),
            param_embeddings: ndarray::Array2::zeros((0, 1)),
            slot_card_embeddings: None,
            slot_card_labels: None,
        };

        let bad_path = unique_temp_path("bad_registry", "mcr");
        bad_registry.save(&bad_path).expect("write bad registry");
        let bad_path_c = CString::new(bad_path.to_string_lossy().into_owned()).unwrap();

        let rc = unsafe { spectre_load_registry(handle, bad_path_c.as_ptr(), &mut err) };
        assert_eq!(rc, 3, "expected dimension mismatch to fail with code 3");

        unsafe {
            spectre_close(handle);
            if !err.is_null() {
                spectre_free_string(err);
            }
        }
        let _ = std::fs::remove_file(bad_path);
    }

    fn unique_temp_path(prefix: &str, ext: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{}_{}.{}", prefix, nanos, ext))
    }
}
