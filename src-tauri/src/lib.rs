mod commands;
mod midi;
mod tray;

use commands::BridgeHandle;
use midi::bridge::Bridge;
use parking_lot::Mutex;
use tauri::{Manager, RunEvent, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let bridge = Bridge::new(app.handle().clone());
            app.manage(BridgeHandle(Mutex::new(bridge)));
            tray::setup(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_midi_ports,
            commands::start_bridge,
            commands::stop_bridge,
            commands::send_sysex,
            commands::get_status,
            commands::run_stream_test,
        ])
        .build(tauri::generate_context!())
        .expect("error while building application")
        .run(|app, event| {
            if let RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } = &event
            {
                api.prevent_close();
                if let Some(window) = app.get_webview_window(label) {
                    let _ = window.hide();
                }
            }
        });
}
