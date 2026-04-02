use crate::FfiHandle;
use rustler::{Env, NifResult, ResourceArc, Term};
use std::path::Path;
use std::sync::Mutex;

struct NifHandle(Mutex<FfiHandle>);
impl rustler::Resource for NifHandle {}

rustler::init!("Elixir.Spectre.FFI", load = on_load);

fn on_load(env: Env, _info: Term) -> bool {
    env.register::<NifHandle>().is_ok()
}

fn nif_err<E: std::fmt::Display>(err: E) -> rustler::Error {
    rustler::Error::Term(Box::new(err.to_string()))
}

#[rustler::nif(schedule = "DirtyCpu")]
fn open(model_dir: String, registry_mcr: String) -> NifResult<ResourceArc<NifHandle>> {
    let (_meta, embedder) = spectre_core::pack::load_pack(Path::new(&model_dir)).map_err(nif_err)?;
    let compiled = spectre_core::CompiledRegistry::load(Path::new(&registry_mcr)).map_err(nif_err)?;
    let dispatcher = spectre_core::SpectreDispatcher::new(embedder, compiled);
    Ok(ResourceArc::new(NifHandle(Mutex::new(FfiHandle { dispatcher }))))
}

#[rustler::nif(schedule = "DirtyCpu")]
fn plan_json(handle: ResourceArc<NifHandle>, request_json: String) -> NifResult<String> {
    let request: spectre_core::PlanRequest = serde_json::from_str(&request_json).map_err(nif_err)?;
    let h = handle.0.lock().unwrap();
    let call_plan = h.dispatcher.plan(&request);
    serde_json::to_string(&call_plan).map_err(nif_err)
}

#[rustler::nif(schedule = "DirtyCpu")]
fn plan_al(handle: ResourceArc<NifHandle>, al_text: String) -> NifResult<String> {
    let h = handle.0.lock().unwrap();
    let call_plan = h.dispatcher.plan_al(&al_text, None, None, None);
    serde_json::to_string(&call_plan).map_err(nif_err)
}

#[rustler::nif(schedule = "DirtyCpu")]
fn plan(handle: ResourceArc<NifHandle>, al_text: String) -> NifResult<String> {
    let h = handle.0.lock().unwrap();
    let call_plan = h.dispatcher.plan_al(&al_text, None, None, None);
    serde_json::to_string(&call_plan).map_err(nif_err)
}

#[rustler::nif(schedule = "DirtyCpu")]
fn add_action(handle: ResourceArc<NifHandle>, action_json: String) -> NifResult<bool> {
    let action: spectre_core::types::ToolDef = serde_json::from_str(&action_json).map_err(nif_err)?;
    let mut h = handle.0.lock().unwrap();
    h.dispatcher.add_action(action).map_err(nif_err)?;
    Ok(true)
}

#[rustler::nif(schedule = "DirtyCpu")]
fn delete_action(handle: ResourceArc<NifHandle>, action_id: String) -> NifResult<bool> {
    let mut h = handle.0.lock().unwrap();
    h.dispatcher.delete_action(&action_id).map_err(nif_err)
}

#[rustler::nif(schedule = "DirtyCpu")]
fn load_registry(handle: ResourceArc<NifHandle>, registry_mcr: String) -> NifResult<bool> {
    let mut h = handle.0.lock().unwrap();
    h.dispatcher.set_registry(Path::new(&registry_mcr)).map_err(nif_err)?;
    Ok(true)
}

#[rustler::nif]
fn action_count(handle: ResourceArc<NifHandle>) -> u64 {
    let h = handle.0.lock().unwrap();
    h.dispatcher.action_count() as u64
}

#[rustler::nif]
fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
