use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;

struct FfiHandle {
    dispatcher: spectre_core::SpectreDispatcher,
}

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

#[no_mangle]
pub unsafe extern "C" fn spectre_close(handle: *mut FfiHandle) {
    if handle.is_null() {
        return;
    }
    let _boxed: Box<FfiHandle> = Box::from_raw(handle);
}

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

    rustler::init!("Elixir.Spectre.FFI", [open, plan_json], load = on_load);

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
        let request: spectre_core::PlanRequest = serde_json::from_str(&request_json)
            .map_err(|e| rustler::Error::Term(Box::new(format!("{}", e))))?;
        let h = handle.0.lock().unwrap();
        let plan = h.dispatcher.plan(&request);
        serde_json::to_string(&plan)
            .map_err(|e| rustler::Error::Term(Box::new(format!("{}", e))))
    }
}
