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

use audio::engine::AudioEngine;
use audio::{
    AudioDeviceInfo, AudioEngineCommand, FxRouting, KnobMappingInfo, KnobTarget, PadEngineMode,
    PadPlaybackMode, PadSampleData, PadSlotInfo, PatternBank, QuantizeMode, SequencerAction,
    SequencerBankToggleResult, SequencerStateInfo, StepResolution, MAX_STEPS, NUM_PADS,
};
use midi::parser::parse_midi_message;
use midi::{MidiManager, MidiPortInfo};
use soundfont::{
    delete_user_soundfont, import_soundfont_file, load_soundfont_from_file, scan_soundfonts,
    SoundFontInfo,
};
use state::{AppState, AudioStatusInfo};

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

#[tauri::command]
fn load_soundfont(filename: String, state: State<'_, AppState>) -> Result<SoundFontInfo, String> {
    let (sf_arc, info) = load_soundfont_from_file(&filename)?;
    let default_preset = info.presets.first();
    let default_patch = default_preset.map(|p| p.patch).unwrap_or(0);
    let default_bank = default_preset.map(|p| p.bank).unwrap_or(0);

    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::LoadSoundFont {
            sound_font: sf_arc,
            patch_number: default_patch,
            bank_number: default_bank,
        });
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SelectPreset {
            patch_number,
            bank_number: bank,
        });
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
fn list_audio_devices() -> Vec<AudioDeviceInfo> {
    AudioEngine::list_output_devices()
}

#[tauri::command]
fn get_audio_status(state: State<'_, AppState>) -> AudioStatusInfo {
    let engine = state.audio_engine.lock().unwrap();
    AudioStatusInfo {
        device_name: engine.current_device_name.lock().unwrap().clone(),
        sample_rate: *engine.sample_rate.lock().unwrap(),
        buffer_size: *engine.buffer_size.lock().unwrap(),
        is_running: engine.is_running.load(std::sync::atomic::Ordering::SeqCst),
    }
}

#[tauri::command]
fn set_master_volume(gain: f32, state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetMasterGain(gain.clamp(0.0, 1.0)));
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetPadEngineMode { mode });
    }
    if let Ok(mut m) = engine.shared_pad_engine_mode.lock() {
        *m = mode;
    }
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

        if let Ok(mut prod) = state.audio_producer.lock() {
            let _ = prod.push(AudioEngineCommand::LoadPadSample {
                pad_index,
                sample: arc_sample.clone(),
            });
        }

        if let Ok(mut slots) = engine.shared_pad_samples.lock() {
            slots[pad_index] = Some(arc_sample.clone());
        }

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

    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::LoadPadSample {
            pad_index,
            sample: arc_sample.clone(),
        });
    }

    if let Ok(mut slots) = engine.shared_pad_samples.lock() {
        slots[pad_index] = Some(arc_sample.clone());
    }

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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::ClearPadSample { pad_index });
    }
    if let Ok(mut slots) = engine.shared_pad_samples.lock() {
        slots[pad_index] = None;
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetPadMode { pad_index, mode });
    }
    if let Ok(mut modes) = engine.shared_pad_modes.lock() {
        modes[pad_index] = mode;
    }
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

    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetPadGain { pad_index, gain });
    }
    if let Ok(mut gains) = engine.shared_pad_gains.lock() {
        gains[pad_index] = gain;
    }
    Ok(())
}

#[tauri::command]
fn set_drum_pad_note(pad_index: usize, note: i32, state: State<'_, AppState>) -> Result<(), String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetDrumPadNote { pad_index, note });
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut notes) = engine.shared_drum_pad_notes.lock() {
        notes[pad_index] = note;
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetMidiLearnCcMappings { faders, knobs });
        for (i, &note) in pads.iter().enumerate() {
            let _ = prod.push(AudioEngineCommand::SetDrumPadNote { pad_index: i, note });
        }
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut pad_notes) = engine.shared_drum_pad_notes.lock() {
        *pad_notes = pads;
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetKnobMapping { knob_index, target });
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut fx) = engine.shared_fx_rack.lock() {
        fx.set_knob_mapping(knob_index, target);
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetKnobValue {
            knob_index,
            value: val_clamped,
        });
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut fx) = engine.shared_fx_rack.lock() {
        fx.set_knob_value(knob_index, val_clamped);
    }
    Ok(())
}

#[tauri::command]
fn get_fx_routing(state: State<'_, AppState>) -> FxRouting {
    let engine = state.audio_engine.lock().unwrap();
    engine.get_fx_routing()
}

#[tauri::command]
fn set_fx_routing(routing: FxRouting, state: State<'_, AppState>) -> Result<FxRouting, String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetFxRouting(routing));
    }
    let engine = state.audio_engine.lock().unwrap();
    engine.set_fx_routing(routing);
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerPlay);
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.play();
    }
    Ok(())
}

#[tauri::command]
fn sequencer_stop(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerStop);
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.stop();
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerTogglePlay);
    }
    Ok(is_playing)
}

#[tauri::command]
fn sequencer_set_bpm(bpm: f32, state: State<'_, AppState>) -> Result<(), String> {
    let clamped = bpm.clamp(40.0, 300.0);
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerSetBpm(clamped));
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_bpm(clamped);
    }
    Ok(())
}

#[tauri::command]
fn sequencer_adjust_bpm(delta: f32, state: State<'_, AppState>) -> Result<f32, String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerAdjustBpm(delta));
    }
    let engine = state.audio_engine.lock().unwrap();
    let new_bpm = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.adjust_bpm(delta);
        seq.bpm
    } else {
        120.0
    };
    Ok(new_bpm)
}

#[tauri::command]
fn sequencer_tap_tempo(state: State<'_, AppState>) -> Result<Option<f32>, String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerTapTempo);
    }
    let engine = state.audio_engine.lock().unwrap();
    let tempo = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.tap_tempo()
    } else {
        None
    };
    Ok(tempo)
}

#[tauri::command]
fn sequencer_set_record(recording: bool, state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerSetRecord(recording));
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_recording(recording);
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerToggleRecord);
    }
    Ok(rec)
}

#[tauri::command]
fn sequencer_set_step(
    pad_index: usize,
    step_index: usize,
    velocity: u8,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if pad_index >= 8 || step_index >= 32 {
        return Err("Índice de paso o pad fuera de rango".to_string());
    }
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerSetStep {
            pad_index,
            step_index,
            velocity,
        });
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_step(pad_index, step_index, velocity);
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerToggleStep {
            pad_index,
            step_index,
        });
    }
    let engine = state.audio_engine.lock().unwrap();
    let new_val = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.toggle_step(pad_index, step_index)
    } else {
        0
    };
    Ok(new_val)
}

#[tauri::command]
fn sequencer_clear(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerClear);
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.clear_pattern();
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerSetResolution(res));
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_resolution(res);
    }
    Ok(res)
}

#[tauri::command]
fn sequencer_toggle_click(state: State<'_, AppState>) -> Result<bool, String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerToggleClick);
    }
    let engine = state.audio_engine.lock().unwrap();
    let enabled = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.toggle_click()
    } else {
        false
    };
    Ok(enabled)
}

#[tauri::command]
fn sequencer_toggle_bank(state: State<'_, AppState>) -> Result<SequencerBankToggleResult, String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerToggleBank);
    }
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
    Ok(res)
}

#[tauri::command]
fn sequencer_pause(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerPause);
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.pause();
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerSetSelectedPad(pad_index));
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_selected_pad(pad_index);
    }
    Ok(())
}

#[tauri::command]
fn sequencer_clear_selected_track(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerClearSelectedTrack);
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.clear_selected_track();
    }
    Ok(())
}

#[tauri::command]
fn sequencer_clear_track(pad_index: usize, state: State<'_, AppState>) -> Result<(), String> {
    if pad_index >= 8 {
        return Err("Índice de pad inválido".to_string());
    }
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerClearTrack(pad_index));
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.clear_track(pad_index);
    }
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
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerSetQuantizeMode(q_mode));
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_quantize_mode(q_mode);
    }
    Ok(q_mode)
}

#[tauri::command]
fn sequencer_set_quantize_strength(
    strength: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clamped = strength.clamp(0.0, 1.0);
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerSetQuantizeStrength(clamped));
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_quantize_strength(clamped);
    }
    Ok(())
}

#[tauri::command]
fn sequencer_requantize(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerRequantize);
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.requantize_pattern();
    }
    Ok(())
}

#[tauri::command]
fn sequencer_set_latency_compensation(
    ticks: i32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clamped = ticks.clamp(-96, 96);
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerSetLatencyCompensation(clamped));
    }
    let engine = state.audio_engine.lock().unwrap();
    if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.set_latency_compensation_ticks(clamped);
    }
    Ok(())
}

#[tauri::command]
fn sequencer_undo(state: State<'_, AppState>) -> Result<bool, String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SequencerUndo);
    }
    let engine = state.audio_engine.lock().unwrap();
    let undone = if let Ok(mut seq) = engine.shared_sequencer.lock() {
        seq.undo()
    } else {
        false
    };
    Ok(undone)
}

#[tauri::command]
fn set_midi_mute(muted: bool, state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut prod) = state.audio_producer.lock() {
        let _ = prod.push(AudioEngineCommand::SetMidiMute(muted));
    }
    Ok(())
}

fn main() {
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
            list_audio_devices,
            get_audio_status,
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
