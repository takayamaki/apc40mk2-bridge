use midir::{MidiInput, MidiInputConnection, MidiOutput};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use super::sysex::ApcMode;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum BridgeState {
    Stopped,
    Running,
    Reconnecting,
}

pub struct Bridge {
    state: Arc<Mutex<BridgeState>>,
    connection: Arc<Mutex<Option<MidiInputConnection<()>>>>,
    stop_flag: Arc<AtomicBool>,
    input_port: Arc<Mutex<String>>,
    output_port: Arc<Mutex<String>>,
    app_handle: AppHandle,
}

impl Bridge {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            state: Arc::new(Mutex::new(BridgeState::Stopped)),
            connection: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            input_port: Arc::new(Mutex::new("loopMIDI Port".into())),
            output_port: Arc::new(Mutex::new("APC40 mkII".into())),
            app_handle,
        }
    }

    pub fn state(&self) -> BridgeState {
        self.state.lock().clone()
    }

    pub fn input_port_name(&self) -> String {
        self.input_port.lock().clone()
    }

    pub fn output_port_name(&self) -> String {
        self.output_port.lock().clone()
    }

    pub fn set_input_port(&self, name: String) {
        *self.input_port.lock() = name;
    }

    pub fn set_output_port(&self, name: String) {
        *self.output_port.lock() = name;
    }

    pub fn start(&self) -> Result<(), String> {
        let current = self.state.lock().clone();
        if current == BridgeState::Running || current == BridgeState::Reconnecting {
            return Err("Bridge is already running".into());
        }

        let input_name = self.input_port.lock().clone();
        let output_name = self.output_port.lock().clone();

        self.connect_bridge(&input_name, &output_name)?;
        self.stop_flag.store(false, Ordering::SeqCst);
        self.start_watchdog();
        Ok(())
    }

    fn connect_bridge(&self, input_port_name: &str, output_port_name: &str) -> Result<(), String> {
        // Close existing connection if any
        if let Some(c) = self.connection.lock().take() {
            c.close();
        }

        let midi_out = MidiOutput::new("apc40mk2-bridge-out").map_err(|e| e.to_string())?;
        let out_port = midi_out
            .ports()
            .into_iter()
            .find(|p| {
                midi_out
                    .port_name(p)
                    .map(|n| n == output_port_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("Output port not found: {}", output_port_name))?;

        let mut out_conn = midi_out
            .connect(&out_port, "apc40mk2-bridge-out")
            .map_err(|e| e.to_string())?;

        let mode_msg = ApcMode::AlternateAbleton.sysex_message();
        out_conn.send(&mode_msg).map_err(|e| e.to_string())?;

        let midi_in = MidiInput::new("apc40mk2-bridge-in").map_err(|e| e.to_string())?;
        let in_port = midi_in
            .ports()
            .into_iter()
            .find(|p| {
                midi_in
                    .port_name(p)
                    .map(|n| n == input_port_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("Input port not found: {}", input_port_name))?;

        let out_conn = Arc::new(Mutex::new(out_conn));
        let out_conn_cb = Arc::clone(&out_conn);
        let app_handle = self.app_handle.clone();

        let in_conn = midi_in
            .connect(
                &in_port,
                "apc40mk2-bridge-in",
                move |timestamp_us, message, _| {
                    let _ = out_conn_cb.lock().send(message);
                    let _ = app_handle.emit(
                        "midi-event",
                        MidiEventPayload {
                            direction: "out",
                            bytes: message.to_vec(),
                            timestamp_us,
                        },
                    );
                },
                (),
            )
            .map_err(|e| e.to_string())?;

        *self.connection.lock() = Some(in_conn);
        *self.state.lock() = BridgeState::Running;
        let _ = self.app_handle.emit("bridge-status", "Running");

        Ok(())
    }

    fn start_watchdog(&self) {
        let state = Arc::clone(&self.state);
        let connection = Arc::clone(&self.connection);
        let stop_flag = Arc::clone(&self.stop_flag);
        let input_port = Arc::clone(&self.input_port);
        let output_port = Arc::clone(&self.output_port);
        let app_handle = self.app_handle.clone();

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(2));

                if stop_flag.load(Ordering::SeqCst) {
                    return;
                }

                let current_state = state.lock().clone();
                let out_name = output_port.lock().clone();
                let in_name = input_port.lock().clone();

                let out_exists = port_exists_output(&out_name);
                let in_exists = port_exists_input(&in_name);

                match current_state {
                    BridgeState::Running => {
                        if !out_exists || !in_exists {
                            if let Some(c) = connection.lock().take() {
                                c.close();
                            }
                            *state.lock() = BridgeState::Reconnecting;
                            let _ = app_handle.emit("bridge-status", "Reconnecting...");
                        }
                    }
                    BridgeState::Reconnecting => {
                        if out_exists && in_exists {
                            let midi_out =
                                match MidiOutput::new("apc40mk2-bridge-out") {
                                    Ok(m) => m,
                                    Err(_) => continue,
                                };
                            let out_port_handle = match midi_out.ports().into_iter().find(|p| {
                                midi_out
                                    .port_name(p)
                                    .map(|n| n == out_name)
                                    .unwrap_or(false)
                            }) {
                                Some(p) => p,
                                None => continue,
                            };
                            let mut out_conn = match midi_out
                                .connect(&out_port_handle, "apc40mk2-bridge-out")
                            {
                                Ok(c) => c,
                                Err(_) => continue,
                            };

                            let mode_msg = ApcMode::AlternateAbleton.sysex_message();
                            let _ = out_conn.send(&mode_msg);

                            let midi_in = match MidiInput::new("apc40mk2-bridge-in") {
                                Ok(m) => m,
                                Err(_) => continue,
                            };
                            let in_port_handle = match midi_in.ports().into_iter().find(|p| {
                                midi_in
                                    .port_name(p)
                                    .map(|n| n == in_name)
                                    .unwrap_or(false)
                            }) {
                                Some(p) => p,
                                None => continue,
                            };

                            let out_conn = Arc::new(Mutex::new(out_conn));
                            let out_conn_cb = Arc::clone(&out_conn);
                            let ah = app_handle.clone();

                            let in_conn = match midi_in.connect(
                                &in_port_handle,
                                "apc40mk2-bridge-in",
                                move |timestamp_us, message, _| {
                                    let _ = out_conn_cb.lock().send(message);
                                    let _ = ah.emit(
                                        "midi-event",
                                        MidiEventPayload {
                                            direction: "out",
                                            bytes: message.to_vec(),
                                            timestamp_us,
                                        },
                                    );
                                },
                                (),
                            ) {
                                Ok(c) => c,
                                Err(_) => continue,
                            };

                            *connection.lock() = Some(in_conn);
                            *state.lock() = BridgeState::Running;
                            let _ = app_handle.emit("bridge-status", "Running");
                        }
                    }
                    BridgeState::Stopped => {
                        return;
                    }
                }
            }
        });
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(c) = self.connection.lock().take() {
            c.close();
        }
        *self.state.lock() = BridgeState::Stopped;
        let _ = self.app_handle.emit("bridge-status", "Stopped");
    }

    pub fn send_sysex(&self, data: &[u8]) -> Result<(), String> {
        let output_name = self.output_port.lock().clone();
        let midi_out = MidiOutput::new("apc40mk2-bridge-sysex").map_err(|e| e.to_string())?;
        let out_port = midi_out
            .ports()
            .into_iter()
            .find(|p| {
                midi_out
                    .port_name(p)
                    .map(|n| n == output_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("Output port not found: {}", output_name))?;

        let mut conn = midi_out
            .connect(&out_port, "apc40mk2-bridge-sysex")
            .map_err(|e| e.to_string())?;

        conn.send(data).map_err(|e| e.to_string())?;
        conn.close();
        Ok(())
    }
}

fn port_exists_output(name: &str) -> bool {
    let midi_out = match MidiOutput::new("apc40mk2-bridge-check") {
        Ok(m) => m,
        Err(_) => return false,
    };
    midi_out
        .ports()
        .iter()
        .any(|p| midi_out.port_name(p).map(|n| n == name).unwrap_or(false))
}

fn port_exists_input(name: &str) -> bool {
    let midi_in = match MidiInput::new("apc40mk2-bridge-check") {
        Ok(m) => m,
        Err(_) => return false,
    };
    midi_in
        .ports()
        .iter()
        .any(|p| midi_in.port_name(p).map(|n| n == name).unwrap_or(false))
}

#[derive(serde::Serialize, Clone)]
struct MidiEventPayload {
    direction: &'static str,
    bytes: Vec<u8>,
    timestamp_us: u64,
}
