//! Commands for checking the app from outside, in debug builds only.

use std::sync::Arc;

use tauri::{AppHandle, Manager as _, State};
use tauri_runtime_cef::CefRuntime;

use crate::{
    app::App,
    screen::{self, Pointer},
};

/// Routes `pointer` as the main window would have, on the main thread where
/// its events arrive, so a check can click, drag, and zoom on the native
/// canvas.
#[tauri::command]
pub fn debug_pointer(
    handle: AppHandle<CefRuntime>,
    app: State<'_, Arc<App>>,
    pointer: Pointer,
    ctrl: bool,
) -> Result<(), String> {
    let app = Arc::clone(&app);
    handle
        .run_on_main_thread(move || screen::feed(&app, pointer, ctrl))
        .map_err(|error| error.to_string())
}
