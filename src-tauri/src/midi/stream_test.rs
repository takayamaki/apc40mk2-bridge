/// Experiment: send MIDI via midiStreamOut (timestamped scheduler) instead of
/// midiOutShortMsg (immediate). If midiStreamOut with delta=0 fails but
/// midiOutShortMsg succeeds, the Windows MIDI Services 2026 stream scheduler
/// is the culprit for Resolume's LED feedback failure.

#[cfg(target_os = "windows")]
mod winmm {
    use std::ffi::c_void;

    pub type HMIDISTRM = *mut c_void;
    pub type MMRESULT = u32;

    pub const MMSYSERR_NOERROR: u32 = 0;
    pub const CALLBACK_NULL: u32 = 0x00000000;

    #[repr(C)]
    pub struct MidiOutCapsW {
        pub w_mid: u16,
        pub w_pid: u16,
        pub v_driver_version: u32,
        pub sz_pname: [u16; 32],
        pub w_technology: u16,
        pub w_voices: u16,
        pub w_notes: u16,
        pub w_channel_mask: u16,
        pub dw_support: u32,
    }

    #[repr(C)]
    pub struct MidiHdr {
        pub lp_data: *mut u8,
        pub dw_buffer_length: u32,
        pub dw_bytes_recorded: u32,
        pub dw_user: usize,
        pub dw_flags: u32,
        pub lp_next: *mut MidiHdr,
        pub reserved: usize,
        pub dw_offset: u32,
        pub dw_reserved: [usize; 8],
    }

    #[link(name = "winmm")]
    extern "system" {
        pub fn midiOutGetNumDevs() -> u32;
        pub fn midiOutGetDevCapsW(
            u_device_id: usize,
            pmoc: *mut MidiOutCapsW,
            cb_moc: u32,
        ) -> MMRESULT;
        pub fn midiStreamOpen(
            phms: *mut HMIDISTRM,
            pu_device_id: *mut u32,
            c_midi: u32,
            dw_callback: usize,
            dw_instance: usize,
            fdw_open: u32,
        ) -> MMRESULT;
        pub fn midiStreamClose(hms: HMIDISTRM) -> MMRESULT;
        pub fn midiStreamRestart(hms: HMIDISTRM) -> MMRESULT;
        pub fn midiStreamOut(
            hms: HMIDISTRM,
            pmh: *mut MidiHdr,
            cbmh: u32,
        ) -> MMRESULT;
        pub fn midiOutPrepareHeader(
            hmo: *mut c_void,
            pmh: *mut MidiHdr,
            cbmh: u32,
        ) -> MMRESULT;
        pub fn midiOutUnprepareHeader(
            hmo: *mut c_void,
            pmh: *mut MidiHdr,
            cbmh: u32,
        ) -> MMRESULT;
    }

    pub fn mmresult_name(r: MMRESULT) -> &'static str {
        match r {
            0 => "MMSYSERR_NOERROR",
            1 => "MMSYSERR_ERROR",
            2 => "MMSYSERR_BADDEVICEID",
            4 => "MMSYSERR_ALLOCATED",
            6 => "MMSYSERR_NODRIVER",
            7 => "MMSYSERR_NOMEM",
            8 => "MMSYSERR_NOTSUPPORTED",
            11 => "MMSYSERR_INVALPARAM",
            64 => "MIDIERR_UNPREPARED",
            65 => "MIDIERR_STILLPLAYING",
            67 => "MIDIERR_NOTREADY",
            68 => "MIDIERR_NODEVICE",
            _ => "UNKNOWN",
        }
    }
}

/// Heap-pinned MIDI event buffer that stays alive until explicitly dropped.
/// Prevents use-after-free: midiStreamOut holds a pointer to the buffer data
/// and reads it asynchronously when the scheduled tick arrives.
#[cfg(target_os = "windows")]
struct PinnedMidiBuffer {
    event_data: Box<[u32; 3]>,
    hdr: Box<winmm::MidiHdr>,
}

#[cfg(target_os = "windows")]
impl PinnedMidiBuffer {
    fn new(delta: u32, event: u32) -> Self {
        Self {
            event_data: Box::new([delta, 0, event]),
            hdr: Box::new(unsafe { std::mem::zeroed() }),
        }
    }

    fn prepare_and_send(&mut self, hstream: winmm::HMIDISTRM) -> Result<(), String> {
        use std::mem;
        use winmm::*;

        self.hdr.lp_data = self.event_data.as_mut_ptr() as *mut u8;
        self.hdr.dw_buffer_length = 12;
        self.hdr.dw_bytes_recorded = 12;

        let r = unsafe {
            midiOutPrepareHeader(
                hstream as *mut _,
                &mut *self.hdr,
                mem::size_of::<MidiHdr>() as u32,
            )
        };
        if r != MMSYSERR_NOERROR {
            return Err(format!("PrepareHeader: {} ({})", mmresult_name(r), r));
        }

        let r = unsafe {
            midiStreamOut(hstream, &mut *self.hdr, mem::size_of::<MidiHdr>() as u32)
        };
        if r != MMSYSERR_NOERROR {
            unsafe {
                midiOutUnprepareHeader(
                    hstream as *mut _,
                    &mut *self.hdr,
                    mem::size_of::<MidiHdr>() as u32,
                );
            }
            return Err(format!("StreamOut: {} ({})", mmresult_name(r), r));
        }

        Ok(())
    }

    fn unprepare(&mut self, hstream: winmm::HMIDISTRM) {
        unsafe {
            winmm::midiOutUnprepareHeader(
                hstream as *mut _,
                &mut *self.hdr,
                std::mem::size_of::<winmm::MidiHdr>() as u32,
            );
        }
    }
}

#[cfg(target_os = "windows")]
pub fn run(device_name: &str) -> Result<Vec<String>, String> {
    use std::mem;
    use winmm::*;

    let mut log: Vec<String> = Vec::new();

    // --- enumerate output devices ---
    let num_devs = unsafe { midiOutGetNumDevs() };
    log.push(format!("MIDI output devices: {}", num_devs));

    let mut device_id: Option<u32> = None;
    for i in 0..num_devs {
        let mut caps: MidiOutCapsW = unsafe { mem::zeroed() };
        let r = unsafe {
            midiOutGetDevCapsW(i as usize, &mut caps, mem::size_of::<MidiOutCapsW>() as u32)
        };
        if r == MMSYSERR_NOERROR {
            let end = caps.sz_pname.iter().position(|&c| c == 0).unwrap_or(32);
            let name = String::from_utf16_lossy(&caps.sz_pname[..end]);
            log.push(format!("  [{}] {}", i, name));
            if name == device_name {
                device_id = Some(i);
            }
        }
    }

    let dev_id = device_id.ok_or_else(|| format!("Device not found: {}", device_name))?;
    log.push(format!("Target device ID: {}", dev_id));

    // --- open stream ---
    let mut hstream: HMIDISTRM = std::ptr::null_mut();
    let mut dev_id_val = dev_id;
    let r = unsafe {
        midiStreamOpen(&mut hstream, &mut dev_id_val, 1, 0, 0, CALLBACK_NULL)
    };
    if r != MMSYSERR_NOERROR {
        let msg = format!("midiStreamOpen FAILED: {} ({})", mmresult_name(r), r);
        log.push(msg.clone());
        if r == 4 {
            log.push("Device is already open — stop the bridge first".into());
        }
        return Err(msg);
    }
    log.push("midiStreamOpen: OK".into());

    let r = unsafe { midiStreamRestart(hstream) };
    if r != MMSYSERR_NOERROR {
        log.push(format!("midiStreamRestart FAILED: {} ({})", mmresult_name(r), r));
        unsafe { midiStreamClose(hstream); }
        return Err(format!("midiStreamRestart failed: {}", r));
    }
    log.push("midiStreamRestart: OK".into());

    // All buffers are heap-pinned and stay alive until after midiStreamClose.
    // MIDIEVENT dwEvent: status | (data1 << 8) | (data2 << 16) | (MEVT << 24)
    // APC40 mk2 palette: 0x05=red, 0x15=green, 0x25=cyan, 0x35=pink
    let mut buffers: Vec<PinnedMidiBuffer> = Vec::new();

    // --- Test A: delta=0 (immediate) ---
    log.push("--- Test A: delta=0 (immediate) ---".into());
    let mut buf_a = PinnedMidiBuffer::new(0, 0x00050090);
    match buf_a.prepare_and_send(hstream) {
        Ok(()) => log.push("OK — pad 0 should light RED now".into()),
        Err(e) => log.push(format!("FAILED — {}", e)),
    }
    buffers.push(buf_a);

    std::thread::sleep(std::time::Duration::from_secs(1));

    // --- Test B: delta=1 (~20ms after A) ---
    log.push("--- Test B: delta=1 (~20ms after A) ---".into());
    let mut buf_b = PinnedMidiBuffer::new(1, 0x00150190);
    match buf_b.prepare_and_send(hstream) {
        Ok(()) => log.push("OK — pad 1 should light GREEN now".into()),
        Err(e) => log.push(format!("FAILED — {}", e)),
    }
    buffers.push(buf_b);

    std::thread::sleep(std::time::Duration::from_secs(1));

    // --- Test C: delta=240 (~5s from stream start) ---
    // Cumulative: A@tick0 + B@tick1 + C@tick241. Stream clock ~2s ≈ tick96.
    // So C fires ~3s from now.
    log.push("--- Test C: delta=240 (~5s from stream start) ---".into());
    let mut buf_c = PinnedMidiBuffer::new(240, 0x00250290);
    match buf_c.prepare_and_send(hstream) {
        Ok(()) => log.push("Queued — pad 2 should light CYAN at ~5s mark".into()),
        Err(e) => log.push(format!("FAILED — {}", e)),
    }
    buffers.push(buf_c);

    // --- Test D: delta=0 (same tick as C) ---
    log.push("--- Test D: delta=0 (same tick as C) ---".into());
    let mut buf_d = PinnedMidiBuffer::new(0, 0x00350390);
    match buf_d.prepare_and_send(hstream) {
        Ok(()) => log.push("Queued — pad 3 should light PINK at same time as C".into()),
        Err(e) => log.push(format!("FAILED — {}", e)),
    }
    buffers.push(buf_d);

    log.push("Waiting 8s — watch for C+D lighting SIMULTANEOUSLY ~3s from now".into());
    std::thread::sleep(std::time::Duration::from_secs(8));

    // --- cleanup: unprepare all buffers, then close stream ---
    for buf in &mut buffers {
        buf.unprepare(hstream);
    }
    unsafe { midiStreamClose(hstream); }
    log.push("Stream closed".into());
    log.push("Expected results:".into());
    log.push("  A: red lights immediately".into());
    log.push("  B: green lights ~1s later (thread::sleep, not scheduling)".into());
    log.push("  C+D at ~5s mark: cyan+pink simultaneously = D blocked behind C".into());
    log.push("  C only, no D: scheduler dropped D".into());
    log.push("  Neither C nor D: stream froze".into());

    Ok(log)
}

#[cfg(not(target_os = "windows"))]
pub fn run(_device_name: &str) -> Result<Vec<String>, String> {
    Err("Stream test is only available on Windows".into())
}
