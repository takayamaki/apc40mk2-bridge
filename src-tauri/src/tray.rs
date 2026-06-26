use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{TrayIconBuilder, TrayIconId},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

use crate::midi::{self, sysex::ApcMode};

const TRAY_ID: &str = "main-tray";

pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.state::<crate::commands::BridgeHandle>();
    let current_input = handle.0.lock().input_port_name();
    let current_output = handle.0.lock().output_port_name();

    let menu = build_menu(app, &current_input, &current_output)?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon not set");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip(format!("APC40mk2 Bridge — {} → {}", current_input, current_output))
        .menu(&menu)
        .on_menu_event(handle_menu_event)
        .build(app)?;

    Ok(())
}

pub fn refresh_menu(app: &AppHandle) {
    let handle = app.state::<crate::commands::BridgeHandle>();
    let current_input = handle.0.lock().input_port_name();
    let current_output = handle.0.lock().output_port_name();

    let menu = match build_menu(app, &current_input, &current_output) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to build menu: {}", e);
            return;
        }
    };

    let tray_id = TrayIconId::new(TRAY_ID);
    if let Some(tray) = app.tray_by_id(&tray_id) {
        let _ = tray.set_menu(Some(menu));
        let _ = tray.set_tooltip(Some(&format!(
            "APC40mk2 Bridge — {} → {}",
            current_input, current_output
        )));
    }
}

fn build_menu(
    app: &AppHandle,
    current_input: &str,
    current_output: &str,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let handle = app.state::<crate::commands::BridgeHandle>();
    let current_state = handle.0.lock().state();
    let is_running = current_state != crate::midi::bridge::BridgeState::Stopped;

    let start = MenuItemBuilder::with_id("start", "Start Bridge")
        .enabled(!is_running)
        .build(app)?;
    let stop = MenuItemBuilder::with_id("stop", "Stop Bridge")
        .enabled(is_running)
        .build(app)?;

    // Input port submenu
    let input_ports = midi::list_input_ports();
    let mut input_sub = SubmenuBuilder::with_id(app, "input_ports", "Input Port");
    for port in &input_ports {
        let label = if *port == current_input {
            format!("● {}", port)
        } else {
            port.clone()
        };
        let item = MenuItemBuilder::with_id(format!("in:{}", port), &label).build(app)?;
        input_sub = input_sub.item(&item);
    }
    let input_sub = input_sub.build()?;

    // Output port submenu
    let output_ports = midi::list_output_ports();
    let mut output_sub = SubmenuBuilder::with_id(app, "output_ports", "Output Port");
    for port in &output_ports {
        let label = if *port == current_output {
            format!("● {}", port)
        } else {
            port.clone()
        };
        let item = MenuItemBuilder::with_id(format!("out:{}", port), &label).build(app)?;
        output_sub = output_sub.item(&item);
    }
    let output_sub = output_sub.build()?;

    let refresh = MenuItemBuilder::with_id("refresh", "Refresh Ports").build(app)?;

    let mode2 = MenuItemBuilder::with_id("mode2", ApcMode::AlternateAbleton.label()).build(app)?;
    let mode1 = MenuItemBuilder::with_id("mode1", ApcMode::Ableton.label()).build(app)?;
    let mode0 = MenuItemBuilder::with_id("mode0", ApcMode::Generic.label()).build(app)?;
    let mode_menu = SubmenuBuilder::with_id(app, "modes", "Send Mode SysEx")
        .item(&mode2)
        .item(&mode1)
        .item(&mode0)
        .build()?;

    let stream_test =
        MenuItemBuilder::with_id("stream_test", "Test: midiStreamOut").build(app)?;
    let debug = MenuItemBuilder::with_id("debug", "Open Debug Monitor").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&start)
        .item(&stop)
        .separator()
        .item(&input_sub)
        .item(&output_sub)
        .item(&refresh)
        .separator()
        .item(&mode_menu)
        .separator()
        .item(&stream_test)
        .item(&debug)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&quit)
        .build()?;

    Ok(menu)
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref().to_string();
    match id.as_str() {
        "start" => {
            let handle = app.state::<crate::commands::BridgeHandle>();
            if let Err(e) = handle.0.lock().start() {
                eprintln!("Failed to start bridge: {}", e);
            }
            refresh_menu(app);
        }
        "stop" => {
            let handle = app.state::<crate::commands::BridgeHandle>();
            handle.0.lock().stop();
            refresh_menu(app);
        }
        "refresh" => {
            refresh_menu(app);
        }
        "mode0" => send_mode(app, ApcMode::Generic),
        "mode1" => send_mode(app, ApcMode::Ableton),
        "mode2" => send_mode(app, ApcMode::AlternateAbleton),
        "stream_test" => {
            run_stream_test(app);
        }
        "debug" => {
            open_debug_window(app);
        }
        "quit" => {
            let handle = app.state::<crate::commands::BridgeHandle>();
            handle.0.lock().stop();
            app.exit(0);
        }
        other => {
            if let Some(port_name) = other.strip_prefix("in:") {
                let handle = app.state::<crate::commands::BridgeHandle>();
                handle.0.lock().set_input_port(port_name.to_string());
                refresh_menu(app);
            } else if let Some(port_name) = other.strip_prefix("out:") {
                let handle = app.state::<crate::commands::BridgeHandle>();
                handle.0.lock().set_output_port(port_name.to_string());
                refresh_menu(app);
            }
        }
    }
}

fn send_mode(app: &AppHandle, mode: ApcMode) {
    let handle = app.state::<crate::commands::BridgeHandle>();
    let msg = mode.sysex_message();
    let result = handle.0.lock().send_sysex(&msg);
    if let Err(e) = result {
        eprintln!("Failed to send SysEx: {}", e);
    }
}

fn run_stream_test(app: &AppHandle) {
    open_debug_window(app);
    let handle = app.state::<crate::commands::BridgeHandle>();
    let output_name = handle.0.lock().output_port_name();
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = app.emit(
            "stream-test-log",
            vec!["Starting midiStreamOut test...".to_string()],
        );
        match crate::midi::stream_test::run(&output_name) {
            Ok(log) => {
                let _ = app.emit("stream-test-log", log);
            }
            Err(e) => {
                let _ = app.emit("stream-test-log", vec![format!("ERROR: {}", e)]);
            }
        }
    });
}

fn open_debug_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("debug") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let _ = WebviewWindowBuilder::new(app, "debug", WebviewUrl::default())
        .title("APC40mk2 Bridge - Debug Monitor")
        .inner_size(600.0, 400.0)
        .build();
}
