use parking_lot::Mutex;
use tauri::State;

use crate::midi::{self, bridge::Bridge};

pub struct BridgeHandle(pub Mutex<Bridge>);

#[tauri::command]
pub fn list_midi_ports() -> (Vec<String>, Vec<String>) {
    (midi::list_input_ports(), midi::list_output_ports())
}

#[tauri::command]
pub fn start_bridge(state: State<'_, BridgeHandle>) -> Result<(), String> {
    state.0.lock().start()
}

#[tauri::command]
pub fn stop_bridge(state: State<'_, BridgeHandle>) {
    state.0.lock().stop();
}

#[tauri::command]
pub fn send_sysex(data: Vec<u8>, state: State<'_, BridgeHandle>) -> Result<(), String> {
    state.0.lock().send_sysex(&data)
}

#[tauri::command]
pub fn get_status(state: State<'_, BridgeHandle>) -> String {
    format!("{:?}", state.0.lock().state())
}

#[tauri::command]
pub fn run_stream_test(device: String) -> Result<Vec<String>, String> {
    midi::stream_test::run(&device)
}
