use std::sync::{Arc, Mutex};
use rtrb::Producer;
use serde::{Deserialize, Serialize};

use crate::audio::{engine::AudioEngine, AudioEngineCommand};
use crate::midi::MidiManager;
use crate::soundfont::SoundFontInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatusInfo {
    pub device_name: String,
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub is_running: bool,
}

pub struct AppState {
    pub midi_manager: Arc<Mutex<MidiManager>>,
    pub audio_engine: Arc<Mutex<AudioEngine>>,
    pub audio_producer: Arc<Mutex<Producer<AudioEngineCommand>>>,
    pub current_soundfont: Arc<Mutex<Option<SoundFontInfo>>>,
}
