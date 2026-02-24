use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;

pub struct FfiHandle {
    dispatcher: spectre_core::SpectreDispatcher,
}

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

#[cfg(feature = "rustler")]
mod nif {
    use super::*;
    use rustler::{Env, NifResult, ResourceArc, Term};
    use std::sync::Mutex;

    struct NifHandle(Mutex<FfiHandle>);

    rustler::init!("Elixir.Spectre.FFI", [open, plan_json, plan_al], load = on_load);

    fn on_load(env: Env, _info: Term) -> bool {
        rustler::resource!(NifHandle, env);
        true
    }

    #[rustler::nif(schedule = "DirtyCpu")]
    fn open(model_dir: String, registry_mcr: String) -> NifResult<ResourceArc<NifHandle>> {
        let (_meta, embedder) = spectre_core::pack::load_pack(Path::new(&model_dir))
            .map_err(|e| rustler::Error::Term(Box::new(format!("{}", e))))?;
        let compiled = spectre_core::CompiledRegistry::load(Path::new(&registry_mcr))
            .map_err(|e| rustler::Error::Term(Box::new(format!("{}", e))))?;
        let dispatcher = spectre_core::SpectreDispatcher::new(embedder, compiled);
        Ok(ResourceArc::new(NifHandle(Mutex::new(FfiHandle { dispatcher }))))
    }

    #[rustler::nif(schedule = "DirtyCpu")]
    fn plan_json(handle: ResourceArc<NifHandle>, request_json: String) -> NifResult<String> {
        let request: spectre_core::PlanRequest =
            serde_json::from_str(&request_json).map_err(|e| rustler::Error::Term(Box::new(format!("{}", e))))?;
        let h = handle.0.lock().unwrap();
        let plan = h.dispatcher.plan(&request);
        serde_json::to_string(&plan).map_err(|e| rustler::Error::Term(Box::new(format!("{}", e))))
    }

    #[rustler::nif(schedule = "DirtyCpu")]
    fn plan_al(handle: ResourceArc<NifHandle>, al_text: String) -> NifResult<String> {
        let h = handle.0.lock().unwrap();
        let plan = h.dispatcher.plan_al(&al_text, None, None, None);
        serde_json::to_string(&plan).map_err(|e| rustler::Error::Term(Box::new(format!("{}", e))))
    }
}
