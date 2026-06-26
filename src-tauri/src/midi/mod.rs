pub mod bridge;
pub mod stream_test;
pub mod sysex;

use midir::{MidiInput, MidiOutput};

pub fn list_input_ports() -> Vec<String> {
    let midi_in = match MidiInput::new("apc-bridge-scan") {
        Ok(m) => m,
        Err(_) => return vec![],
    };
    midi_in
        .ports()
        .iter()
        .filter_map(|p| midi_in.port_name(p).ok())
        .collect()
}

pub fn list_output_ports() -> Vec<String> {
    let midi_out = match MidiOutput::new("apc-bridge-scan") {
        Ok(m) => m,
        Err(_) => return vec![],
    };
    midi_out
        .ports()
        .iter()
        .filter_map(|p| midi_out.port_name(p).ok())
        .collect()
}
