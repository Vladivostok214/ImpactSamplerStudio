// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod midi;
mod soundfont;
mod state;

use std::path::Path;
use std::sync::{Arc, Mutex};
use rtrb::RingBuffer;
use tauri::{AppHandle, Emitter, Manager, State};

use audio::engine::{pack_bank_patch, AudioEngine};
use audio::{
    ArpMode, ArpRate, ArpStateInfo, AudioDeviceInfo, AudioEngineCommand, FxRouting,
    KnobMappingInfo, KnobTarget, PadEngineMode, PadPlaybackMode, PadSampleData, PadSlotInfo,
    PatternBank, QuantizeMode, SequencerAction, SequencerBankToggleResult, SequencerStateInfo,
    StepResolution, MAX_STEPS, NUM_PADS,
};
use midi::parser::parse_midi_message;
use midi::{MidiManager, MidiPortInfo};
use soundfont::{
    delete_user_soundfont, import_soundfont_file, load_soundfont_from_file, scan_soundfonts,
    SoundFontInfo,
};
use state::{AppState, AudioDeviceDetails, AudioStatusInfo};

#[tauri::command]
fn show_main_window(window: tauri::WebviewWindow) {
    let _ = window.show();
}

#[tauri::command]
fn list_soundfonts() -> Vec<SoundFontInfo> {
    scan_soundfonts()
}

#[tauri::command]
fn pick_and_import_soundfont() -> Result<Option<SoundFontInfo>, String> {
    let file = rfd::FileDialog::new()
        .add_filter("SoundFont 2 (*.sf2)", &["sf2"])
        .set_title("Seleccionar Instrumento SoundFont (.sf2)")
        .pick_file();

    if let Some(path) = file {
        let info = import_soundfont_file(&path)?;
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

#[tauri::command]
fn import_soundfont_from_path(file_path: String) -> Result<SoundFontInfo, String> {
    let path = Path::new(&file_path);
    import_soundfont_file(path)
}

#[tauri::command]
fn remove_user_soundfont(filename: String) -> Result<(), String> {
    delete_user_soundfont(&filename)
}

/// Política explícita de encolado para comandos persistentes de configuración (A06 — R1/R2/R5):
/// - Debe invocarse bajo la misma sección crítica que actualizó `shared_*` (manteniendo el orden de locks
///   `state.audio_engine` -> `state.audio_producer`) para impedir inversión de orden entre hilos IPC.
/// - Empaqueta el comando como `AudioEngineCommand::IpcPreApplied(Box::new(cmd))`:
///   - Durante el streaming activo (`process_commands`), el hilo de audio lo desenvuelve y lo aplica al
///     estado local de renderizado.
///   - Durante una reconstrucción del stream (`reconcile_pending_commands_before_rebuild`), se ignora
///     porque su efecto ya fue aplicado en orden exacto a los espejos `shared_*` desde IPC. Esto impide
///     reejecutar comandos no idempotentes (`TogglePlay`, `ToggleRecord`, `ToggleBank`, `Undo`, `Clear`,
///     `Requantize`) y evita que comandos IPC antiguos en una cola llena sobrescriban un estado más
///     reciente en `shared_*`, sin descartar jamás eventos MIDI físicos legítimos de la cola.
fn push_persistent_config_command(
    engine: &AudioEngine,
    prod_mutex: &Mutex<rtrb::Producer<AudioEngineCommand>>,
    cmd: AudioEngineCommand,
    label: &str,
) {
    if let Ok(mut prod) = prod_mutex.lock() {
        if prod
            .push(AudioEngineCommand::IpcPreApplied(Box::new(cmd)))
            .is_err()
        {
            if engine.is_running.load(std::sync::atomic::Ordering::SeqCst) {
                eprintln!(
                    "[Audio IPC Warning] Cola de comandos llena al enviar '{}'. El espejo compartido conserva la configuración más reciente para el próximo rebuild.",
                    label
                );
            }
        }
    }
}

#[tauri::command]
fn load_soundfont(filename: String, state: State<'_, AppState>) -> Result<SoundFontInfo, String> {
    let (sf_arc, info) = load_soundfont_from_file(&filename)?;
    let default_preset = info.presets.first();
    let default_patch = default_preset.map(|p| p.patch).unwrap_or(0);
    let default_bank = default_preset.map(|p| p.bank).unwrap_or(0);

    {
        let engine = state.audio_engine.lock().unwrap();
        if let Ok(mut sf_guard) = engine.shared_soundfont.lock() {
            *sf_guard = Some(sf_arc.clone());
        }
        engine.shared_bank_patch.store(
            pack_bank_patch(default_bank, default_patch),
            std::sync::atomic::Ordering::SeqCst,
        );
        push_persistent_config_command(
            &engine,
            &state.audio_producer,
            AudioEngineCommand::LoadSoundFont {
                sound_font: sf_arc,
                patch_number: default_patch,
                bank_number: default_bank,
            },
            "LoadSoundFont",
        );
    }

    *state.current_soundfont.lock().unwrap() = Some(info.clone());
    Ok(info)
}

#[tauri::command]
fn select_soundfont_preset(
    patch_number: i32,
    bank_number: Option<i32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let bank = bank_number.unwrap_or(0);
    {
        let engine = state.audio_engine.lock().unwrap();
        engine.shared_bank_patch.store(
            pack_bank_patch(bank, patch_number),
            std::sync::atomic::Ordering::SeqCst,
        );
        push_persistent_config_command(
            &engine,
            &state.audio_producer,
            AudioEngineCommand::SelectPreset {
                patch_number,
                bank_number: bank,
            },
            "SelectPreset",
        );
    }
    Ok(())
}

#[tauri::command]
fn get_current_soundfont(state: State<'_, AppState>) -> Option<SoundFontInfo> {
    state.current_soundfont.lock().unwrap().clone()
}

#[tauri::command]
fn list_midi_ports(state: State<'_, AppState>) -> Vec<MidiPortInfo> {
    let midi_mgr = state.midi_manager.lock().unwrap();
    midi_mgr.list_ports()
}

#[tauri::command]
fn select_midi_port(
    index: usize,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<MidiPortInfo>, String> {
    let mut midi_mgr = state.midi_manager.lock().unwrap();
    midi_mgr.connect_to_port(index, app_handle, state.audio_producer.clone())
}

#[tauri::command]
fn connect_midi(app_handle: AppHandle, state: State<'_, AppState>) -> Result<Vec<MidiPortInfo>, String> {
    let mut midi_mgr = state.midi_manager.lock().unwrap();
    midi_mgr.start_auto_connect(app_handle, state.audio_producer.clone())
}

#[tauri::command]
fn rescan_midi_ports(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<MidiPortInfo>, String> {
    let mut midi_mgr = state.midi_manager.lock().unwrap();
    if !midi_mgr.is_connected() {
        let _ = midi_mgr.start_auto_connect(app_handle, state.audio_producer.clone());
    }
    Ok(midi_mgr.list_ports())
}

#[tauri::command]
fn list_audio_hosts() -> Vec<String> {
    if cfg!(target_os = "windows") {
        vec!["WASAPI".to_string(), "ASIO".to_string()]
    } else if cfg!(target_os = "macos") {
        vec!["CoreAudio".to_string()]
    } else {
        vec!["ALSA".to_string()]
    }
}

#[tauri::command]
fn open_asio_control_panel(
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<AudioStatusInfo, String> {
    #[cfg(target_os = "windows")]
    {
        let engine = state.audio_engine.lock().unwrap();
        engine.open_asio_control_panel(device_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (state, device_name);
        Err("ASIO solo está disponible en Windows".to_string())
    }
}

#[tauri::command]
fn get_device_details(
    host: String,
    device_name: String,
    state: State<'_, AppState>,
) -> Result<AudioDeviceDetails, String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.get_device_details(&host, &device_name)
}

#[tauri::command]
fn suspend_audio_engine(state: State<'_, AppState>) -> Result<AudioStatusInfo, String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.suspend_audio_engine()
}

#[tauri::command]
fn resume_audio_engine(state: State<'_, AppState>) -> Result<AudioStatusInfo, String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.resume_audio_engine()
}

#[tauri::command]
fn list_audio_devices(host: Option<String>) -> Vec<AudioDeviceInfo> {
    AudioEngine::list_output_devices(host.as_deref())
}

#[tauri::command]
fn get_audio_status(state: State<'_, AppState>) -> AudioStatusInfo {
    let engine = state.audio_engine.lock().unwrap();
    let current_host = engine.current_host.lock().unwrap().clone();
    AudioStatusInfo {
        host: current_host.clone(),
        device_name: engine.current_device_name.lock().unwrap().clone(),
        driver: current_host,
        channels: *engine.channels.lock().unwrap(),
        sample_rate: *engine.sample_rate.lock().unwrap(),
        buffer_size: *engine.buffer_size.lock().unwrap(),
        is_running: engine.is_running.load(std::sync::atomic::Ordering::SeqCst),
    }
}

#[tauri::command]
fn set_audio_config(
    host: Option<String>,
    device_name: Option<String>,
    channel_pair: usize,
    buffer_size: Option<u32>,
    sample_rate: Option<u32>,
    state: State<'_, AppState>,
) -> Result<AudioStatusInfo, String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.apply_audio_config(host, device_name, channel_pair, buffer_size, sample_rate)
}

#[tauri::command]
fn set_audio_device(
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<AudioStatusInfo, String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.switch_output_device(device_name)
}

#[tauri::command]
fn set_audio_channel_pair(
    pair: usize,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.set_channel_pair(pair);
    Ok(())
}

#[tauri::command]
fn set_audio_buffer_size(
    buffer_size: u32,
    state: State<'_, AppState>,
) -> Result<AudioStatusInfo, String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.set_buffer_size(buffer_size)
}

#[tauri::command]
fn set_audio_sample_rate(
    sample_rate: u32,
    state: State<'_, AppState>,
) -> Result<AudioStatusInfo, String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.set_sample_rate(sample_rate)
}

#[tauri::command]
fn set_master_volume(gain: f32, state: State<'_, AppState>) -> Result<(), String> {
    let clamped = gain.clamp(0.0, 1.0);
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut g) = engine.shared_master_gain.lock() {
        *g = clamped;
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetMasterGain(clamped),
        "SetMasterGain",
    );
    Ok(())
}

#[tauri::command]
fn trigger_virtual_note(
    note: u8,
    velocity: f32,
    is_on: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        if is_on {
            let _ = prod.push(AudioEngineCommand::NoteOn {
                note,
                velocity: velocity.clamp(0.0, 1.0),
            });
        } else {
            let _ = prod.push(AudioEngineCommand::NoteOff { note });
        }
    }

    let vel_byte = if is_on {
        (velocity.clamp(0.0, 1.0) * 127.0).round() as u8
    } else {
        0
    };
    let status_byte = if is_on { 0x90 } else { 0x80 };
    let bytes = [status_byte, note, vel_byte];
    if let Some(event) = parse_midi_message("Virtual Input", &bytes) {
        let _ = app_handle.emit("midi-event", &event);
    }

    Ok(())
}

#[tauri::command]
fn trigger_virtual_cc(
    controller: u8,
    value: f32,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::ControlChange {
            controller,
            value: value.clamp(0.0, 1.0),
        });
    }

    let val_byte = (value.clamp(0.0, 1.0) * 127.0).round() as u8;
    let bytes = [0xB0, controller, val_byte];
    if let Some(event) = parse_midi_message("Virtual Input", &bytes) {
        let _ = app_handle.emit("midi-event", &event);
    }

    Ok(())
}

#[tauri::command]
fn trigger_virtual_pitch_bend(
    normalized: i16,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::PitchBend { normalized });
    }

    let raw = (normalized as i32 + 8192).clamp(0, 16383) as u16;
    let lsb = (raw & 0x7F) as u8;
    let msb = ((raw >> 7) & 0x7F) as u8;
    let bytes = [0xE0, lsb, msb];
    if let Some(event) = parse_midi_message("Virtual Input", &bytes) {
        let _ = app_handle.emit("midi-event", &event);
    }

    Ok(())
}

// --- FASE 3: COMANDOS DE PADS ---

#[tauri::command]
fn get_pad_slots(state: State<'_, AppState>) -> Vec<PadSlotInfo> {
    let engine = state.audio_engine.lock().unwrap();
    engine.get_pad_slots_info()
}

#[tauri::command]
fn get_pad_engine_mode(state: State<'_, AppState>) -> PadEngineMode {
    let engine = state.audio_engine.lock().unwrap();
    engine.get_pad_engine_mode()
}

#[tauri::command]
fn set_pad_engine_mode(mode: PadEngineMode, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut m) = engine.shared_pad_engine_mode.lock() {
        *m = mode;
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetPadEngineMode { mode },
        "SetPadEngineMode",
    );
    Ok(())
}

#[tauri::command]
fn trigger_virtual_pad(
    pad_index: usize,
    velocity: f32,
    is_on: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        if is_on {
            let _ = prod.push(AudioEngineCommand::PadTrigger {
                pad_index,
                velocity: velocity.clamp(0.0, 1.0),
            });
        } else {
            let _ = prod.push(AudioEngineCommand::PadRelease { pad_index });
        }
    }

    let pad_num = (pad_index + 1) as u8;
    let vel_byte = if is_on {
        (velocity.clamp(0.0, 1.0) * 127.0).round() as u8
    } else {
        0
    };
    let note = 36 + pad_index as u8;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let event = midi::parser::NormalizedMidiEvent {
        port_name: "Virtual Pad".to_string(),
        channel: 10,
        event: if is_on {
            midi::parser::MidiEventType::NoteOn {
                note,
                velocity: vel_byte,
                note_name: format!("Pad {}", pad_num),
                pad_index: Some(pad_num),
            }
        } else {
            midi::parser::MidiEventType::NoteOff {
                note,
                velocity: 0,
                note_name: format!("Pad {}", pad_num),
                pad_index: Some(pad_num),
            }
        },
        timestamp_ms: now_ms,
    };
    let _ = app_handle.emit("midi-event", event);

    Ok(())
}

#[tauri::command]
fn pick_and_load_pad_sample(
    pad_index: usize,
    state: State<'_, AppState>,
) -> Result<Option<PadSlotInfo>, String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }

    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Audio WAV", &["wav"])
        .set_title("Seleccionar muestra de audio WAV")
        .pick_file()
    {
        let engine = state.audio_engine.lock().unwrap();
        let target_rate = *engine.sample_rate.lock().unwrap();
        let sample_data = PadSampleData::from_file(&path, target_rate)?;
        let arc_sample = Arc::new(sample_data);

        if let Ok(mut slots) = engine.shared_pad_samples.lock() {
            slots[pad_index] = Some(arc_sample.clone());
        }

        push_persistent_config_command(
            &engine,
            &state.audio_producer,
            AudioEngineCommand::LoadPadSample {
                pad_index,
                sample: arc_sample.clone(),
            },
            "LoadPadSample",
        );

        let mode = engine.shared_pad_modes.lock().unwrap()[pad_index];
        let gain = engine.shared_pad_gains.lock().unwrap()[pad_index];

        Ok(Some(PadSlotInfo {
            index: pad_index,
            pad_number: pad_index + 1,
            sample_name: Some(arc_sample.name.clone()),
            mode,
            gain,
            has_sample: true,
        }))
    } else {
        Ok(None)
    }
}

#[tauri::command]
fn load_pad_sample_from_file(
    pad_index: usize,
    path: String,
    state: State<'_, AppState>,
) -> Result<PadSlotInfo, String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }

    let p = Path::new(&path);
    let engine = state.audio_engine.lock().unwrap();
    let target_rate = *engine.sample_rate.lock().unwrap();
    let sample_data = PadSampleData::from_file(p, target_rate)?;
    let arc_sample = Arc::new(sample_data);

    if let Ok(mut slots) = engine.shared_pad_samples.lock() {
        slots[pad_index] = Some(arc_sample.clone());
    }

    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::LoadPadSample {
            pad_index,
            sample: arc_sample.clone(),
        },
        "LoadPadSample",
    );

    let mode = engine.shared_pad_modes.lock().unwrap()[pad_index];
    let gain = engine.shared_pad_gains.lock().unwrap()[pad_index];

    Ok(PadSlotInfo {
        index: pad_index,
        pad_number: pad_index + 1,
        sample_name: Some(arc_sample.name.clone()),
        mode,
        gain,
        has_sample: true,
    })
}

#[tauri::command]
fn clear_pad_slot(pad_index: usize, state: State<'_, AppState>) -> Result<PadSlotInfo, String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }

    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut slots) = engine.shared_pad_samples.lock() {
        slots[pad_index] = None;
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::ClearPadSample { pad_index },
        "ClearPadSample",
    );
    let mode = engine.shared_pad_modes.lock().unwrap()[pad_index];
    let gain = engine.shared_pad_gains.lock().unwrap()[pad_index];

    Ok(PadSlotInfo {
        index: pad_index,
        pad_number: pad_index + 1,
        sample_name: None,
        mode,
        gain,
        has_sample: false,
    })
}

#[tauri::command]
fn set_pad_slot_mode(
    pad_index: usize,
    mode: PadPlaybackMode,
    state: State<'_, AppState>,
) -> Result<PadSlotInfo, String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }

    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut modes) = engine.shared_pad_modes.lock() {
        modes[pad_index] = mode;
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetPadMode { pad_index, mode },
        "SetPadMode",
    );
    let sample_name = engine.shared_pad_samples.lock().unwrap()[pad_index]
        .as_ref()
        .map(|s| s.name.clone());
    let has_sample = sample_name.is_some();
    let gain = engine.shared_pad_gains.lock().unwrap()[pad_index];

    Ok(PadSlotInfo {
        index: pad_index,
        pad_number: pad_index + 1,
        sample_name,
        mode,
        gain,
        has_sample,
    })
}

#[tauri::command]
fn set_pad_slot_gain(pad_index: usize, gain: f32, state: State<'_, AppState>) -> Result<(), String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }

    let clamped = gain.clamp(0.0, 1.0);
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut gains) = engine.shared_pad_gains.lock() {
        gains[pad_index] = clamped;
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetPadGain {
            pad_index,
            gain: clamped,
        },
        "SetPadGain",
    );
    Ok(())
}

#[tauri::command]
fn set_drum_pad_note(pad_index: usize, note: i32, state: State<'_, AppState>) -> Result<(), String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut notes) = engine.shared_drum_pad_notes.lock() {
        notes[pad_index] = note;
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetDrumPadNote { pad_index, note },
        "SetDrumPadNote",
    );
    Ok(())
}

#[tauri::command]
fn get_drum_pad_notes(state: State<'_, AppState>) -> [i32; 8] {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(notes) = engine.shared_drum_pad_notes.lock() {
        *notes
    } else {
        [36, 38, 42, 46, 39, 45, 48, 49]
    }
}

#[tauri::command]
fn set_midi_learn_mappings(
    faders: [u8; 9],
    knobs: [u8; 8],
    pads: [i32; 8],
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let engine = state.audio_engine.lock().unwrap();
        if let Ok(mut f_guard) = engine.shared_midi_fader_ccs.lock() {
            *f_guard = faders;
        }
        if let Ok(mut k_guard) = engine.shared_midi_knob_ccs.lock() {
            *k_guard = knobs;
        }
        push_persistent_config_command(
            &engine,
            &state.audio_producer,
            AudioEngineCommand::SetMidiLearnCcMappings { faders, knobs },
            "SetMidiLearnCcMappings",
        );
    }
    let mut input_pad_notes = [0u8; 8];
    for (i, &note) in pads.iter().enumerate() {
        input_pad_notes[i] = note.clamp(0, 127) as u8;
    }
    let midi_mgr = state.midi_manager.lock().unwrap();
    midi_mgr.set_pad_note_mappings(input_pad_notes);
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct ActionMidiBinding {
    pub message_type: String, // "cc" | "note"
    pub number: u8,
    pub action: SequencerAction,
}

#[tauri::command]
fn set_action_midi_mappings(
    bindings: Vec<ActionMidiBinding>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut cc_list = Vec::new();
    let mut note_list = Vec::new();
    for b in bindings {
        if b.message_type == "cc" {
            cc_list.push((b.number, b.action));
        } else if b.message_type == "note" {
            note_list.push((b.number, b.action));
        }
    }
    let midi_mgr = state.midi_manager.lock().unwrap();
    midi_mgr.set_action_mappings(&cc_list, &note_list);
    Ok(())
}

#[tauri::command]
fn get_knob_mappings(state: State<'_, AppState>) -> Vec<KnobMappingInfo> {
    let engine = state.audio_engine.lock().unwrap();
    engine.get_knob_mappings()
}

#[tauri::command]
fn set_knob_mapping(
    knob_index: usize,
    target: KnobTarget,
    state: State<'_, AppState>,
) -> Result<Vec<KnobMappingInfo>, String> {
    if knob_index >= 8 {
        return Err("Índice de knob inválido".to_string());
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut fx) = engine.shared_fx_rack.lock() {
        fx.set_knob_mapping(knob_index, target);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetKnobMapping { knob_index, target },
        "SetKnobMapping",
    );
    Ok(engine.get_knob_mappings())
}

#[tauri::command]
fn set_knob_value(
    knob_index: usize,
    value: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if knob_index >= 8 {
        return Err("Índice de knob inválido".to_string());
    }
    let val_clamped = value.clamp(0.0, 1.0);
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut fx) = engine.shared_fx_rack.lock() {
        fx.set_knob_value(knob_index, val_clamped);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetKnobValue {
            knob_index,
            value: val_clamped,
        },
        "SetKnobValue",
    );
    Ok(())
}

#[tauri::command]
fn get_fx_routing(state: State<'_, AppState>) -> FxRouting {
    let engine = state.audio_engine.lock().unwrap();
    engine.get_fx_routing()
}

#[tauri::command]
fn set_fx_routing(routing: FxRouting, state: State<'_, AppState>) -> Result<FxRouting, String> {
    let engine = state.audio_engine.lock().unwrap();
    engine.set_fx_routing(routing);
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetFxRouting(routing),
        "SetFxRouting",
    );
    Ok(routing)
}

// --- FASE 5: COMANDOS DEL SECUENCIADOR ---

#[tauri::command]
fn get_sequencer_state(state: State<'_, AppState>) -> SequencerStateInfo {
    let engine = state.audio_engine.lock().unwrap();
    engine.get_sequencer_state()
}

#[tauri::command]
fn sequencer_play(state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.play();
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerPlay,
        "SequencerPlay",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_stop(state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.stop();
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerStop,
        "SequencerStop",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_toggle_play(state: State<'_, AppState>) -> Result<bool, String> {
    let engine = state.audio_engine.lock().unwrap();
    let is_playing = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.toggle_play()
    } else {
        false
    };
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerTogglePlay,
        "SequencerTogglePlay",
    );
    Ok(is_playing)
}

#[tauri::command]
fn sequencer_set_bpm(bpm: f32, state: State<'_, AppState>) -> Result<(), String> {
    let clamped = bpm.clamp(40.0, 300.0);
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_bpm(clamped);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerSetBpm(clamped),
        "SequencerSetBpm",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_adjust_bpm(delta: f32, state: State<'_, AppState>) -> Result<f32, String> {
    let engine = state.audio_engine.lock().unwrap();
    let new_bpm = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.adjust_bpm(delta);
        seq.bpm
    } else {
        120.0
    };
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerAdjustBpm(delta),
        "SequencerAdjustBpm",
    );
    Ok(new_bpm)
}

#[tauri::command]
fn sequencer_tap_tempo(state: State<'_, AppState>) -> Result<Option<f32>, String> {
    let engine = state.audio_engine.lock().unwrap();
    let tempo = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.tap_tempo()
    } else {
        None
    };
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerTapTempo,
        "SequencerTapTempo",
    );
    Ok(tempo)
}

#[tauri::command]
fn sequencer_set_record(recording: bool, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_recording(recording);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerSetRecord(recording),
        "SequencerSetRecord",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_toggle_record(state: State<'_, AppState>) -> Result<bool, String> {
    let engine = state.audio_engine.lock().unwrap();
    let rec = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.toggle_recording()
    } else {
        false
    };
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerToggleRecord,
        "SequencerToggleRecord",
    );
    Ok(rec)
}

#[tauri::command]
fn sequencer_set_step(
    pad_index: usize,
    step_index: usize,
    velocity: u8,
    save_undo: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if pad_index >= 8 || step_index >= 32 {
        return Err("Índice de paso o pad fuera de rango".to_string());
    }
    let should_save_undo = save_undo.unwrap_or(true);
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_step(pad_index, step_index, velocity, should_save_undo);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerSetStep {
            pad_index,
            step_index,
            velocity,
            save_undo: should_save_undo,
        },
        "SequencerSetStep",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_toggle_step(
    pad_index: usize,
    step_index: usize,
    state: State<'_, AppState>,
) -> Result<u8, String> {
    if pad_index >= 8 || step_index >= 32 {
        return Err("Índice de paso o pad fuera de rango".to_string());
    }
    let engine = state.audio_engine.lock().unwrap();
    let new_val = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.toggle_step(pad_index, step_index)
    } else {
        0
    };
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerToggleStep {
            pad_index,
            step_index,
        },
        "SequencerToggleStep",
    );
    Ok(new_val)
}

#[tauri::command]
fn sequencer_clear(state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.clear_pattern();
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerClear,
        "SequencerClear",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_set_resolution(
    resolution: String,
    state: State<'_, AppState>,
) -> Result<StepResolution, String> {
    let res = match resolution.as_str() {
        "thirtySecond" | "1/32" => StepResolution::ThirtySecond,
        "sixteenth" | "1/16" => StepResolution::Sixteenth,
        _ => StepResolution::Eighth,
    };
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_resolution(res);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerSetResolution(res),
        "SequencerSetResolution",
    );
    Ok(res)
}

#[tauri::command]
fn sequencer_toggle_click(state: State<'_, AppState>) -> Result<bool, String> {
    let engine = state.audio_engine.lock().unwrap();
    let enabled = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.toggle_click()
    } else {
        false
    };
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerToggleClick,
        "SequencerToggleClick",
    );
    Ok(enabled)
}

#[tauri::command]
fn sequencer_toggle_bank(state: State<'_, AppState>) -> Result<SequencerBankToggleResult, String> {
    let engine = state.audio_engine.lock().unwrap();
    let res = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        let bank = seq.toggle_bank();
        let pattern = seq.get_active_pattern();
        SequencerBankToggleResult {
            current_bank: bank,
            pattern,
        }
    } else {
        SequencerBankToggleResult {
            current_bank: PatternBank::A,
            pattern: [[0u8; MAX_STEPS]; NUM_PADS],
        }
    };
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerToggleBank,
        "SequencerToggleBank",
    );
    Ok(res)
}

#[tauri::command]
fn sequencer_pause(state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.pause();
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerPause,
        "SequencerPause",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_execute_action(
    action: SequencerAction,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerExecuteAction(action));
    }
    Ok(())
}

#[tauri::command]
fn sequencer_set_selected_pad(pad_index: usize, state: State<'_, AppState>) -> Result<(), String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_selected_pad(pad_index);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerSetSelectedPad(pad_index),
        "SequencerSetSelectedPad",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_clear_selected_track(state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.clear_selected_track();
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerClearSelectedTrack,
        "SequencerClearSelectedTrack",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_clear_track(pad_index: usize, state: State<'_, AppState>) -> Result<(), String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.clear_track(pad_index);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerClearTrack(pad_index),
        "SequencerClearTrack",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_set_quantize_mode(
    mode: String,
    state: State<'_, AppState>,
) -> Result<QuantizeMode, String> {
    let q_mode = match mode.as_str() {
        "off" => QuantizeMode::Off,
        "eighth" => QuantizeMode::Eighth,
        "sixteenth" => QuantizeMode::Sixteenth,
        "thirtySecond" => QuantizeMode::ThirtySecond,
        _ => return Err("Modo de cuantización no válido".to_string()),
    };
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_quantize_mode(q_mode);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerSetQuantizeMode(q_mode),
        "SequencerSetQuantizeMode",
    );
    Ok(q_mode)
}

#[tauri::command]
fn sequencer_set_quantize_strength(
    strength: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clamped = strength.clamp(0.0, 1.0);
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_quantize_strength(clamped);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerSetQuantizeStrength(clamped),
        "SequencerSetQuantizeStrength",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_requantize(state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.requantize_pattern();
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerRequantize,
        "SequencerRequantize",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_set_latency_compensation(
    ticks: i32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clamped = ticks.clamp(-96, 96);
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_latency_compensation_ticks(clamped);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerSetLatencyCompensation(clamped),
        "SequencerSetLatencyCompensation",
    );
    Ok(())
}

#[tauri::command]
fn sequencer_undo(app_handle: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let engine = state.audio_engine.lock().unwrap();
    let is_running = engine.is_running.load(std::sync::atomic::Ordering::SeqCst);
    let undone = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        let res = seq.undo();
        if res && !is_running {
            let _ = app_handle.emit("sequencer-state-changed", seq.get_state_info());
            let _ = app_handle.emit("sequencer-pattern-updated", seq.get_active_pattern());
        }
        res
    } else {
        false
    };
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SequencerUndo,
        "SequencerUndo",
    );
    Ok(undone)
}

#[tauri::command]
fn set_midi_mute(muted: bool, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    engine
        .shared_midi_muted
        .store(muted, std::sync::atomic::Ordering::SeqCst);
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetMidiMute(muted),
        "SetMidiMute",
    );
    Ok(())
}

// Fase 9: Comandos IPC del Arpegiador
#[tauri::command]
fn get_arp_state(state: State<'_, AppState>) -> Result<ArpStateInfo, String> {
    let engine = state.audio_engine.lock().unwrap();
    Ok(engine.get_arp_state())
}

#[tauri::command]
fn set_arp_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut arp) = engine.shared_arpeggiator.lock() {
        arp.set_enabled(enabled);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetArpEnabled(enabled),
        "SetArpEnabled",
    );
    Ok(())
}

#[tauri::command]
fn set_arp_hold(hold: bool, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut arp) = engine.shared_arpeggiator.lock() {
        arp.set_hold(hold);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetArpHold(hold),
        "SetArpHold",
    );
    Ok(())
}

#[tauri::command]
fn set_arp_mode(mode: ArpMode, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut arp) = engine.shared_arpeggiator.lock() {
        arp.set_mode(mode);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetArpMode(mode),
        "SetArpMode",
    );
    Ok(())
}

#[tauri::command]
fn set_arp_rate(rate: ArpRate, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut arp) = engine.shared_arpeggiator.lock() {
        arp.set_rate(rate);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetArpRate(rate),
        "SetArpRate",
    );
    Ok(())
}

#[tauri::command]
fn set_arp_octaves(octaves: u8, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut arp) = engine.shared_arpeggiator.lock() {
        arp.set_octaves(octaves);
    }
    push_persistent_config_command(
        &engine,
        &state.audio_producer,
        AudioEngineCommand::SetArpOctaves(octaves),
        "SetArpOctaves",
    );
    Ok(())
}

fn main() {
    #[cfg(target_os = "linux")]
    crate::audio::engine::silence_alsa_logging();

    let (producer, consumer) = RingBuffer::<AudioEngineCommand>::new(1024);
    let audio_engine = Arc::new(Mutex::new(AudioEngine::new()));

    // Iniciar motor de audio inmediatamente
    if let Err(e) = audio_engine.lock().unwrap().start(consumer) {
        eprintln!("Aviso al iniciar motor de audio: {}", e);
    }

    let app_state = AppState {
        midi_manager: Arc::new(Mutex::new(MidiManager::new())),
        audio_engine: audio_engine.clone(),
        audio_producer: Arc::new(Mutex::new(producer)),
        current_soundfont: Arc::new(Mutex::new(None)),
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            list_midi_ports,
            connect_midi,
            select_midi_port,
            list_audio_hosts,
            open_asio_control_panel,
            get_device_details,
            suspend_audio_engine,
            resume_audio_engine,
            list_audio_devices,
            get_audio_status,
            set_audio_config,
            set_audio_device,
            set_audio_channel_pair,
            set_audio_buffer_size,
            set_audio_sample_rate,
            set_master_volume,
            trigger_virtual_note,
            trigger_virtual_cc,
            trigger_virtual_pitch_bend,
            list_soundfonts,
            pick_and_import_soundfont,
            import_soundfont_from_path,
            remove_user_soundfont,
            load_soundfont,
            select_soundfont_preset,
            get_current_soundfont,
            // Fase 3
            get_pad_slots,
            get_pad_engine_mode,
            set_pad_engine_mode,
            trigger_virtual_pad,
            pick_and_load_pad_sample,
            load_pad_sample_from_file,
            clear_pad_slot,
            set_pad_slot_mode,
            set_pad_slot_gain,
            set_drum_pad_note,
            get_drum_pad_notes,
            // Fase 4: Knobs & FX
            get_knob_mappings,
            set_knob_mapping,
            set_knob_value,
            get_fx_routing,
            set_fx_routing,
            // Fase 5: Secuenciador
            get_sequencer_state,
            sequencer_play,
            sequencer_stop,
            sequencer_toggle_play,
            sequencer_set_bpm,
            sequencer_adjust_bpm,
            sequencer_tap_tempo,
            sequencer_set_record,
            sequencer_toggle_record,
            sequencer_set_step,
            sequencer_toggle_step,
            sequencer_clear,
            sequencer_set_resolution,
            sequencer_toggle_click,
            sequencer_toggle_bank,
            sequencer_pause,
            sequencer_execute_action,
            sequencer_set_selected_pad,
            sequencer_clear_selected_track,
            sequencer_clear_track,
            sequencer_set_quantize_mode,
            sequencer_set_quantize_strength,
            sequencer_requantize,
            sequencer_set_latency_compensation,
            sequencer_undo,
            set_midi_mute,
            show_main_window,
            set_midi_learn_mappings,
            set_action_midi_mappings,
            rescan_midi_ports,
            // Fase 9: Arpeggiator
            get_arp_state,
            set_arp_enabled,
            set_arp_hold,
            set_arp_mode,
            set_arp_rate,
            set_arp_octaves,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            let state = app.state::<AppState>();

            // Inicializar directorio de recursos de Tauri para resolución de SoundFonts y Samples en producción
            if let Ok(res_dir) = app.path().resource_dir() {
                soundfont::init_resource_dir(Some(res_dir));
            }

            // Inicializar directorio de datos de usuario para guardar SoundFonts importados
            if let Ok(data_dir) = app.path().app_data_dir() {
                soundfont::init_user_data_dir(Some(data_dir));
            }

            // Cargar o refrescar samples de fábrica y batería en el motor de audio con la ruta oficial resuelta
            if let Ok(mut prod) = state.audio_producer.lock() {
                let engine = state.audio_engine.lock().unwrap();
                engine.reload_factory_assets(&mut prod);
            }

            let mut midi_mgr = state.midi_manager.lock().unwrap();
            let _ = midi_mgr.start_auto_connect(app_handle.clone(), state.audio_producer.clone());
            drop(midi_mgr);

            // Iniciar MIDI Watchdog de baja frecuencia para auto-recovery en caliente (0% CPU)
            MidiManager::start_watchdog(
                state.midi_manager.clone(),
                app_handle.clone(),
                state.audio_producer.clone(),
            );

            state.audio_engine.lock().unwrap().set_app_handle(app_handle);

            // Fallback de seguridad: asegurar que la ventana se muestre en máximo 1.5s si el frontend tarda
            if let Some(win) = app.get_webview_window("main") {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let _ = win.show();
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error al ejecutar aplicación Tauri");
}
