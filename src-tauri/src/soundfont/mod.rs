use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use rustysynth::SoundFont;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundFontPresetInfo {
    pub index: usize,
    pub name: String,
    pub bank: i32,
    pub patch: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundFontInfo {
    pub id: String,
    pub filename: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub file_size_mb: f32,
    pub presets: Vec<SoundFontPresetInfo>,
}

pub fn get_soundfonts_dir() -> PathBuf {
    if Path::new("soundfonts").exists() {
        PathBuf::from("soundfonts")
    } else if Path::new("../soundfonts").exists() {
        PathBuf::from("../soundfonts")
    } else {
        PathBuf::from("soundfonts")
    }
}

pub fn scan_soundfonts() -> Vec<SoundFontInfo> {
    let dir = get_soundfonts_dir();
    let mut list = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "sf2") {
                let filename = path.file_name().unwrap().to_string_lossy().to_string();
                let file_size_mb = match entry.metadata() {
                    Ok(m) => (m.len() as f32) / (1024.0 * 1024.0),
                    Err(_) => 0.0,
                };

                // Identificar metadatos según nombre de archivo
                let (display_name, description, category) = if filename.to_lowercase().contains("piano") && !filename.to_lowercase().contains("rhodes") {
                    (
                        "Acoustic Grand Piano".to_string(),
                        "Piano de cola dinámico, muestreo realista con respuesta cristalina.".to_string(),
                        "Acoustic Piano".to_string(),
                    )
                } else if filename.to_lowercase().contains("rhodes") {
                    (
                        "Vintage Rhodes Mark I".to_string(),
                        "Piano eléctrico cálido estilo 70s, armónicos ricos, campanas y tacto sedoso.".to_string(),
                        "Electric Piano".to_string(),
                    )
                } else if filename.to_lowercase().contains("mellotron") {
                    (
                        "Classic Mellotron Tape".to_string(),
                        "Texturas analógicas de cinta magnética, cuerdas vintage, metales y coros.".to_string(),
                        "Tape Synth / Strings".to_string(),
                    )
                } else {
                    (
                        filename.replace(".sf2", ""),
                        "Instrumento SoundFont 2 personalizado.".to_string(),
                        "Custom SF2".to_string(),
                    )
                };

                let id = filename
                    .to_lowercase()
                    .replace(' ', "-")
                    .replace(".sf2", "");

                list.push(SoundFontInfo {
                    id,
                    filename,
                    display_name,
                    description,
                    category,
                    file_size_mb: (file_size_mb * 10.0).round() / 10.0,
                    presets: Vec::new(),
                });
            }
        }
    }

    // Ordenar para que aparezcan en orden: Piano, Rhodes, Mellotron
    list.sort_by(|a, b| {
        let order = |name: &str| {
            if name.contains("Piano") && !name.contains("Rhodes") {
                0
            } else if name.contains("Rhodes") {
                1
            } else if name.contains("Mellotron") {
                2
            } else {
                3
            }
        };
        order(&a.display_name).cmp(&order(&b.display_name))
    });

    list
}

pub fn load_soundfont_from_file(filename: &str) -> Result<(Arc<SoundFont>, SoundFontInfo), String> {
    let dir = get_soundfonts_dir();
    let file_path = dir.join(filename);

    if !file_path.exists() {
        return Err(format!("Archivo no encontrado: {:?}", file_path));
    }

    let mut file = File::open(&file_path)
        .map_err(|e| format!("Error abriendo {:?}: {}", file_path, e))?;

    let sf = SoundFont::new(&mut file)
        .map_err(|e| format!("Error parseando SoundFont {:?}: {:?}", file_path, e))?;

    let sf_arc = Arc::new(sf);

    let presets: Vec<SoundFontPresetInfo> = sf_arc
        .get_presets()
        .iter()
        .enumerate()
        .map(|(idx, p)| SoundFontPresetInfo {
            index: idx,
            name: p.get_name().to_string(),
            bank: p.get_bank_number() as i32,
            patch: p.get_patch_number() as i32,
        })
        .collect();

    let file_size_mb = match std::fs::metadata(&file_path) {
        Ok(m) => (m.len() as f32) / (1024.0 * 1024.0),
        Err(_) => 0.0,
    };

    let (display_name, description, category) = if filename.to_lowercase().contains("piano") && !filename.to_lowercase().contains("rhodes") {
        (
            "Acoustic Grand Piano".to_string(),
            "Piano de cola dinámico, muestreo realista con respuesta cristalina.".to_string(),
            "Acoustic Piano".to_string(),
        )
    } else if filename.to_lowercase().contains("rhodes") {
        (
            "Vintage Rhodes Mark I".to_string(),
            "Piano eléctrico cálido estilo 70s, armónicos ricos, campanas y tacto sedoso.".to_string(),
            "Electric Piano".to_string(),
        )
    } else if filename.to_lowercase().contains("mellotron") {
        (
            "Classic Mellotron Tape".to_string(),
            "Texturas analógicas de cinta magnética, cuerdas vintage, metales y coros.".to_string(),
            "Tape Synth / Strings".to_string(),
        )
    } else {
        (
            filename.replace(".sf2", ""),
            "Instrumento SoundFont 2 personalizado.".to_string(),
            "Custom SF2".to_string(),
        )
    };

    let id = filename
        .to_lowercase()
        .replace(' ', "-")
        .replace(".sf2", "");

    let info = SoundFontInfo {
        id,
        filename: filename.to_string(),
        display_name,
        description,
        category,
        file_size_mb: (file_size_mb * 10.0).round() / 10.0,
        presets,
    };

    Ok((sf_arc, info))
}
