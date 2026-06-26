/// APC40 mk2 model ID
const MODEL_ID: u8 = 0x29;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApcMode {
    Generic,
    Ableton,
    AlternateAbleton,
}

impl ApcMode {
    pub fn label(&self) -> &'static str {
        match self {
            ApcMode::Generic => "Mode 0: Generic",
            ApcMode::Ableton => "Mode 1: Ableton",
            ApcMode::AlternateAbleton => "Mode 2: Alt Ableton",
        }
    }

    fn mode_byte(&self) -> u8 {
        match self {
            ApcMode::Generic => 0x40,
            ApcMode::Ableton => 0x41,
            ApcMode::AlternateAbleton => 0x42,
        }
    }

    /// Build the mode switch SysEx message (including F0/F7 framing).
    pub fn sysex_message(&self) -> Vec<u8> {
        vec![
            0xF0,
            0x47,
            0x7F,
            MODEL_ID,
            0x60,
            0x00,
            0x04,
            self.mode_byte(),
            0x08,
            0x02,
            0x01,
            0xF7,
        ]
    }
}
