use std::sync::{Arc, Mutex};
use rtrb::Producer;
use serde::{Deserialize, Serialize};

use crate::audio::{engine::AudioEngine, AudioEngineCommand};
use crate::midi::MidiManager;
use crate::soundfont::SoundFontInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatusInfo {
    pub host: String,
    pub device_name: String,
    pub driver: String,
    pub channels: usize,
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub is_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceDetails {
    pub name: String,
    pub is_available: bool,
    pub channels: usize,
    pub sample_rates: Vec<u32>,
    pub current_sample_rate: u32,
    pub current_buffer_size: u32,
}

pub struct AppState {
    pub midi_manager: Arc<Mutex<MidiManager>>,
    pub audio_engine: Arc<Mutex<AudioEngine>>,
    pub audio_producer: Arc<Mutex<Producer<AudioEngineCommand>>>,
    pub current_soundfont: Arc<Mutex<Option<SoundFontInfo>>>,
}
