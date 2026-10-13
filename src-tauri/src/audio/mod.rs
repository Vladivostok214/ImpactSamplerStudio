pub mod engine;
pub mod fx;
pub mod sampler;
pub mod sequencer;

use std::sync::Arc;
use rustysynth::SoundFont;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
pub use fx::{AudioFxRack, FxRouting, KnobMappingInfo, KnobTarget};
#[allow(unused_imports)]
pub use sampler::{PadEngineMode, PadPlaybackMode, PadSampleData, PadSlotInfo};
#[allow(unused_imports)]
pub use sequencer::{
    PatternBank, SequencerBankToggleResult, SequencerStateInfo, SequencerStepEvent, StepResolution,
    StepSequencer, NUM_PADS, NUM_STEPS,
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AudioEngineCommand {
    NoteOn { note: u8, velocity: f32 },
    NoteOff { note: u8 },
    ControlChange { controller: u8, value: f32 },
    PitchBend { normalized: i16 },
    SetMasterGain(f32),
    LoadSoundFont { sound_font: Arc<SoundFont>, patch_number: i32 },
    SelectPreset { patch_number: i32 },
    // Fase 3: Pad Commands
    PadTrigger { pad_index: usize, velocity: f32 },
    PadRelease { pad_index: usize },
    LoadPadSample { pad_index: usize, sample: Arc<PadSampleData> },
    ClearPadSample { pad_index: usize },
    SetPadMode { pad_index: usize, mode: PadPlaybackMode },
    SetPadGain { pad_index: usize, gain: f32 },
    SetPadEngineMode { mode: PadEngineMode },
    SetDrumPadNote { pad_index: usize, note: i32 },
    LoadDrumKitSoundFont { sound_font: Arc<SoundFont>, patch_number: i32 },
    SetMidiLearnCcMappings { faders: [u8; 9], knobs: [u8; 8] },
    // Fase 4: Knob & FX Commands
    SetKnobValue { knob_index: usize, value: f32 },
    SetKnobMapping { knob_index: usize, target: KnobTarget },
    SetFxRouting(FxRouting),
    // Fase 5: Sequencer Commands
    SequencerPlay,
    SequencerStop,
    SequencerTogglePlay,
    SequencerSetBpm(f32),
    SequencerAdjustBpm(f32),
    SequencerTapTempo,
    SequencerSetRecord(bool),
    SequencerToggleRecord,
    SequencerSetStep { pad_index: usize, step_index: usize, velocity: u8 },
    SequencerToggleStep { pad_index: usize, step_index: usize },
    SequencerClear,
    SequencerSetResolution(StepResolution),
    SequencerToggleClick,
    SequencerToggleBank,
    SequencerToggleSwing,
    SequencerSetSelectedPad(usize),
    SequencerClearSelectedTrack,
    SequencerClearTrack(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub sample_rates: Vec<u32>,
}
