# APC40mk2 Bridge

A MIDI bridge app that restores LED feedback on the APC40 mk2.

Works around a bug in Windows MIDI Services 2026 where MIDI output (LED control)
from Resolume Avenue to the APC40 mk2 over USB is silently dropped.

## How It Works

```
Resolume (MIDI out) → loopMIDI Port → [APC40mk2 Bridge] → APC40 mk2 (USB)
Resolume (MIDI in)  ← APC40 mk2 (USB)  — direct, unchanged
```

Redirect Resolume's MIDI output to a loopMIDI virtual port.
This app receives those messages and forwards them to the APC40 mk2 immediately.
Input (pad presses → clip triggers) stays direct, so existing mappings work as-is.

Why this works: the bridge uses WinMM `midiOutShortMsg` (immediate send),
bypassing the broken stream scheduler entirely.

## Requirements

- [loopMIDI](https://www.tobias-erichsen.de/software/loopmidi.html) — virtual MIDI port driver
- APC40 mk2
- Resolume Avenue / Arena

## Setup

1. Install **loopMIDI** and create one port (keep the default name `loopMIDI Port`)
2. Launch **APC40mk2 Bridge** (it lives in the system tray)
3. Right-click the tray icon → **Input Port** → select `loopMIDI Port`
4. Right-click the tray icon → **Output Port** → select `APC40 mkII`
5. Click **Start Bridge** → sends Alt Ableton mode SysEx and starts forwarding
6. In **Resolume** MIDI settings:
   - Output: change to `loopMIDI Port`
   - Input: keep `APC40 mkII`

## Features

- **System tray resident**: runs without a window; all control via context menu
- **MIDI port selection**: pick input/output ports from the menu
- **SysEx mode switching**: Mode 0 (Generic) / Mode 1 (Ableton) / Mode 2 (Alt Ableton)
- **Auto-reconnect**: detects APC disconnection and reconnects automatically
- **Debug monitor**: real-time display of forwarded MIDI messages
- **midiStreamOut test**: verifies Windows MIDI Services stream scheduler behavior

## Building

```bash
pnpm install
pnpm tauri build
```

### Prerequisites

- Rust (rustup)
- Node.js + pnpm
- Visual Studio Build Tools 2022
- Windows SDK

## License

[MIT](LICENSE)
