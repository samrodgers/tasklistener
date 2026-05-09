//! C ABI for the SwiftUI / WinUI front-ends.
//!
//! Convention: every function returns either an opaque handle, an `int` status
//! (0 = ok, non-zero = error), or a heap-allocated `*mut c_char`. Strings
//! returned to the caller must be freed with `tl_string_free`. JSON is the
//! canonical wire format for anything beyond a single string — keeps the
//! ABI tiny and easy to evolve.

use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;
use std::sync::Arc;

use tasklistener_core::db::ProviderConfig;
use tasklistener_core::providers;
use tasklistener_core::task::{NewTask, TaskStatus};
use tasklistener_core::{Engine, Event};

static ENGINE: OnceCell<Engine> = OnceCell::new();

type EventCallback = extern "C" fn(json: *const c_char, ctx: *mut std::ffi::c_void);
struct Subscriber {
    cb: EventCallback,
    ctx: usize, // *mut c_void as usize for Send/Sync
}
unsafe impl Send for Subscriber {}
unsafe impl Sync for Subscriber {}

static SUBS: OnceCell<Arc<Mutex<Vec<Subscriber>>>> = OnceCell::new();

fn subs() -> &'static Arc<Mutex<Vec<Subscriber>>> {
    SUBS.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
}

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("TASKLISTENER_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

unsafe fn cstr<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        None
    } else {
        CStr::from_ptr(p).to_str().ok()
    }
}

fn to_cstring<S: Into<String>>(s: S) -> *mut c_char {
    CString::new(s.into()).unwrap_or_default().into_raw()
}

fn json_or_null<T: serde::Serialize>(v: &T) -> *mut c_char {
    serde_json::to_string(v)
        .map(to_cstring)
        .unwrap_or(std::ptr::null_mut())
}

// ---------------------------------------------------------------- engine

/// Start the engine. `db_path` may be NULL to use the platform default.
/// Idempotent — second call is a no-op.
#[no_mangle]
pub unsafe extern "C" fn tl_start(db_path: *const c_char) -> i32 {
    if ENGINE.get().is_some() {
        return 0;
    }
    init_logging();
    let path = match cstr(db_path) {
        Some(s) if !s.is_empty() => PathBuf::from(s),
        _ => tasklistener_core::config::default_db_path(),
    };
    match Engine::start(path) {
        Ok(engine) => {
            // Wire core events through to FFI subscribers.
            engine.subscribe(Box::new(|ev: Event| {
                let json = match serde_json::to_string(&ev) {
                    Ok(j) => j,
                    Err(_) => return,
                };
                let cstr = match CString::new(json) {
                    Ok(c) => c,
                    Err(_) => return,
                };
                let snapshot: Vec<(EventCallback, usize)> = subs()
                    .lock()
                    .iter()
                    .map(|s| (s.cb, s.ctx))
                    .collect();
                for (cb, ctx) in snapshot {
                    cb(cstr.as_ptr(), ctx as *mut _);
                }
            }));
            let _ = ENGINE.set(engine);
            0
        }
        Err(e) => {
            tracing::error!(error = %e, "tl_start failed");
            1
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_subscribe(
    cb: EventCallback,
    ctx: *mut std::ffi::c_void,
) -> i32 {
    subs().lock().push(Subscriber { cb, ctx: ctx as usize });
    0
}

#[no_mangle]
pub unsafe extern "C" fn tl_string_free(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

fn engine_or_null() -> Option<&'static Engine> {
    ENGINE.get()
}

// ---------------------------------------------------------------- tasks

/// Manually capture a task. Bypasses the audio pipeline. Returns the new id
/// as a heap string the caller must free, or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tl_capture_manual(text: *const c_char) -> *mut c_char {
    let Some(engine) = engine_or_null() else { return std::ptr::null_mut() };
    let Some(text) = cstr(text) else { return std::ptr::null_mut() };
    match engine.capture(NewTask::manual(text)) {
        Ok(id) => to_cstring(id),
        Err(e) => {
            tracing::error!(error = %e, "tl_capture_manual");
            std::ptr::null_mut()
        }
    }
}

/// Returns JSON: `{ "tasks": [ Task, ... ] }`. Caller frees with tl_string_free.
#[no_mangle]
pub unsafe extern "C" fn tl_list_tasks(include_done: i32, limit: i64) -> *mut c_char {
    let Some(engine) = engine_or_null() else { return std::ptr::null_mut() };
    let limit = if limit <= 0 { 200 } else { limit };
    let tasks = match engine.db().list_tasks(include_done != 0, limit) {
        Ok(t) => t,
        Err(_) => return std::ptr::null_mut(),
    };
    json_or_null(&serde_json::json!({ "tasks": tasks }))
}

#[no_mangle]
pub unsafe extern "C" fn tl_get_task(id: *const c_char) -> *mut c_char {
    let Some(engine) = engine_or_null() else { return std::ptr::null_mut() };
    let Some(id) = cstr(id) else { return std::ptr::null_mut() };
    let task = match engine.db().get_task(id) {
        Ok(Some(t)) => t,
        _ => return std::ptr::null_mut(),
    };
    let dests = engine.db().list_destinations_for_task(&task.id).unwrap_or_default();
    json_or_null(&serde_json::json!({
        "task": task,
        "destinations": dests,
    }))
}

#[no_mangle]
pub unsafe extern "C" fn tl_update_task_text(id: *const c_char, text: *const c_char) -> i32 {
    let Some(engine) = engine_or_null() else { return 1 };
    let Some(id) = cstr(id) else { return 2 };
    let Some(text) = cstr(text) else { return 2 };
    match engine.db().update_task_text(id, text) {
        Ok(()) => {
            engine.emit(Event::TaskUpdated { task_id: id.to_string() });
            0
        }
        Err(_) => 3,
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_set_task_status(id: *const c_char, status: *const c_char) -> i32 {
    let Some(engine) = engine_or_null() else { return 1 };
    let Some(id) = cstr(id) else { return 2 };
    let Some(status) = cstr(status).and_then(TaskStatus::parse) else { return 2 };
    match engine.db().set_task_status(id, status) {
        Ok(()) => {
            engine.emit(Event::TaskUpdated { task_id: id.to_string() });
            0
        }
        Err(_) => 3,
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_delete_task(id: *const c_char) -> i32 {
    let Some(engine) = engine_or_null() else { return 1 };
    let Some(id) = cstr(id) else { return 2 };
    match engine.db().delete_task(id) {
        Ok(()) => {
            engine.emit(Event::TaskDeleted { task_id: id.to_string() });
            0
        }
        Err(_) => 3,
    }
}

// ---------------------------------------------------------------- providers

/// Save / update a provider config. Body JSON shape:
/// {
///   "id": "todoist:default",
///   "kind": "todoist" | "notion" | "things" | "webhook",
///   "display_name": "...",
///   "enabled": true,
///   "config_json": "{}",
///   "min_confidence": 0.7,
///   "auto_push": true,
///   "target_id": "...",
///   "target_label": "..."
/// }
/// `token` (optional) is written to the keychain for the given provider id.
/// Returns 0 on success.
#[no_mangle]
pub unsafe extern "C" fn tl_set_provider(
    config_json: *const c_char,
    token: *const c_char,
) -> i32 {
    let Some(engine) = engine_or_null() else { return 1 };
    let Some(json) = cstr(config_json) else { return 2 };
    #[derive(serde::Deserialize)]
    struct Body {
        id: String,
        kind: String,
        display_name: String,
        enabled: bool,
        config_json: Option<String>,
        min_confidence: Option<f32>,
        auto_push: Option<bool>,
        target_id: Option<String>,
        target_label: Option<String>,
    }
    let body: Body = match serde_json::from_str(json) {
        Ok(b) => b,
        Err(_) => return 2,
    };
    let mut keychain_ref = None;
    if let Some(tok) = cstr(token) {
        if !tok.is_empty() {
            match tasklistener_core::keychain::Keychain::store(&body.id, tok) {
                Ok(r) => keychain_ref = Some(r),
                Err(_) => return 4,
            }
        }
    }
    let cfg = ProviderConfig {
        id: body.id.clone(),
        kind: body.kind,
        display_name: body.display_name,
        enabled: body.enabled,
        config_json: body.config_json.unwrap_or_else(|| "{}".into()),
        min_confidence: body.min_confidence.unwrap_or(0.7),
        auto_push: body.auto_push.unwrap_or(true),
        last_synced_at: None,
        keychain_ref,
        target_id: body.target_id,
        target_label: body.target_label,
    };
    match engine.db().upsert_provider(&cfg) {
        Ok(()) => {
            engine.emit(Event::ProviderConnected { provider: body.id });
            0
        }
        Err(_) => 3,
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_list_providers() -> *mut c_char {
    let Some(engine) = engine_or_null() else { return std::ptr::null_mut() };
    let providers = match engine.db().list_providers() {
        Ok(p) => p,
        Err(_) => return std::ptr::null_mut(),
    };
    let view: Vec<_> = providers
        .into_iter()
        .map(|p| {
            let masked =
                tasklistener_core::keychain::Keychain::masked_suffix(&p.id).unwrap_or(None);
            serde_json::json!({
                "id": p.id,
                "kind": p.kind,
                "display_name": p.display_name,
                "enabled": p.enabled,
                "config_json": p.config_json,
                "min_confidence": p.min_confidence,
                "auto_push": p.auto_push,
                "target_id": p.target_id,
                "target_label": p.target_label,
                "token_masked": masked,
            })
        })
        .collect();
    json_or_null(&serde_json::json!({ "providers": view }))
}

#[no_mangle]
pub unsafe extern "C" fn tl_delete_provider(id: *const c_char) -> i32 {
    let Some(engine) = engine_or_null() else { return 1 };
    let Some(id) = cstr(id) else { return 2 };
    let _ = tasklistener_core::keychain::Keychain::delete(id);
    match engine.db().delete_provider(id) {
        Ok(()) => {
            engine.emit(Event::ProviderDisconnected { provider: id.to_string() });
            0
        }
        Err(_) => 3,
    }
}

/// Validate token + fetch the provider's targets (lists/projects/databases).
/// Blocks the calling thread on the runtime. Returns JSON `{"targets":[...]}`
/// or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tl_list_targets(provider_id: *const c_char) -> *mut c_char {
    let Some(engine) = engine_or_null() else { return std::ptr::null_mut() };
    let Some(id) = cstr(provider_id) else { return std::ptr::null_mut() };
    let cfg = match engine.db().get_provider(id) {
        Ok(Some(c)) => c,
        _ => return std::ptr::null_mut(),
    };
    let provider = match providers::for_kind(&cfg.kind) {
        Some(p) => p,
        None => return std::ptr::null_mut(),
    };
    let targets = match engine
        .runtime()
        .block_on(async { provider.list_targets(&cfg).await })
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "list_targets failed");
            return std::ptr::null_mut();
        }
    };
    json_or_null(&serde_json::json!({ "targets": targets }))
}

/// Force a push of an existing task to a specific provider, even if auto-push
/// is off or the confidence threshold blocked it.
#[no_mangle]
pub unsafe extern "C" fn tl_push_now(
    task_id: *const c_char,
    provider_id: *const c_char,
) -> i32 {
    let Some(engine) = engine_or_null() else { return 1 };
    let Some(task_id) = cstr(task_id) else { return 2 };
    let Some(provider_id) = cstr(provider_id) else { return 2 };
    if engine.db().enqueue_push(task_id, provider_id).is_err() {
        return 3;
    }
    engine.queue().notify();
    0
}

/// Record a push that was performed by the front-end (e.g. Apple Reminders via
/// EventKit). Either external_id or error must be non-null.
#[no_mangle]
pub unsafe extern "C" fn tl_record_external_push(
    task_id: *const c_char,
    provider_id: *const c_char,
    external_id: *const c_char,
    external_url: *const c_char,
    error: *const c_char,
) -> i32 {
    let Some(engine) = engine_or_null() else { return 1 };
    let Some(task_id) = cstr(task_id) else { return 2 };
    let Some(provider_id) = cstr(provider_id) else { return 2 };
    let ext_id = cstr(external_id);
    let ext_url = cstr(external_url);
    let err = cstr(error);
    match engine
        .db()
        .record_external_push(task_id, provider_id, ext_id, ext_url, err)
    {
        Ok(()) => {
            engine.emit(tasklistener_core::Event::DestinationStateChanged {
                task_id: task_id.to_string(),
                provider: provider_id.to_string(),
                state: if err.is_some() {
                    tasklistener_core::DestinationState::Failed
                } else {
                    tasklistener_core::DestinationState::Pushed
                },
            });
            0
        }
        Err(_) => 3,
    }
}

#[no_mangle]
pub unsafe extern "C" fn tl_audio_is_real() -> i32 {
    if tasklistener_core::audio::is_real() { 1 } else { 0 }
}
