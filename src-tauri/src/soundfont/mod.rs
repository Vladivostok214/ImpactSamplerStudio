use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
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
    #[serde(default)]
    pub is_user: bool,
}

static APP_RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();
static USER_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init_resource_dir(dir: Option<PathBuf>) {
    if let Some(d) = dir {
        let _ = APP_RESOURCE_DIR.set(d);
    }
}

pub fn init_user_data_dir(dir: Option<PathBuf>) {
    if let Some(d) = dir {
        let _ = USER_DATA_DIR.set(d);
    }
}

pub fn get_user_soundfonts_dir() -> PathBuf {
    // 1. Directorio oficial de datos de usuario de Tauri (AppData/Local o asignado)
    if let Some(data_base) = USER_DATA_DIR.get() {
        let dir = data_base.join("soundfonts");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }

    // 2. Fallback estándar para Windows LOCALAPPDATA
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(local_app_data)
            .join("ImpactSamplerStudio")
            .join("soundfonts");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }

    // 3. Fallback de desarrollo local
    let dev_user_dir = if Path::new("soundfonts").exists() {
        PathBuf::from("soundfonts").join("user")
    } else if Path::new("../soundfonts").exists() {
        PathBuf::from("../soundfonts").join("user")
    } else {
        PathBuf::from("soundfonts").join("user")
    };
    let _ = std::fs::create_dir_all(&dev_user_dir);
    dev_user_dir
}

pub fn get_soundfonts_dir() -> PathBuf {
    // 1. Directorio oficial resuelto por Tauri en runtime (bundle instalado)
    if let Some(res_base) = APP_RESOURCE_DIR.get() {
        let sf_sub = res_base.join("soundfonts");
        if sf_sub.exists() {
            return sf_sub;
        }
        if res_base.exists() && (res_base.join("Mellotron.sf2").exists() || res_base.join("Piano - Realtime.sf2").exists()) {
            return res_base.clone();
        }
    }

    // 2. Comprobación junto al binario ejecutable (Windows Installer / Portable)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let res_sub = parent.join("resources").join("soundfonts");
            if res_sub.exists() {
                return res_sub;
            }
            let portable_sub = parent.join("soundfonts");
            if portable_sub.exists() {
                return portable_sub;
            }
        }
    }

    // 3. Modo desarrollo (raíz del workspace o src-tauri)
    if Path::new("soundfonts").exists() {
        PathBuf::from("soundfonts")
    } else if Path::new("../soundfonts").exists() {
        PathBuf::from("../soundfonts")
    } else {
        PathBuf::from("soundfonts")
    }
}

pub fn get_samples_dir() -> PathBuf {
    // 1. Directorio oficial resuelto por Tauri en runtime (bundle instalado)
    if let Some(res_base) = APP_RESOURCE_DIR.get() {
        let sample_sub = res_base.join("samples");
        if sample_sub.exists() {
            return sample_sub;
        }
        if res_base.exists() && res_base.join("01_Kick.wav").exists() {
            return res_base.clone();
        }
    }

    // 2. Comprobación junto al binario ejecutable (Windows Installer / Portable)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let res_sub = parent.join("resources").join("samples");
            if res_sub.exists() {
                return res_sub;
            }
            let portable_sub = parent.join("samples");
            if portable_sub.exists() {
                return portable_sub;
            }
        }
    }

    // 3. Modo desarrollo (raíz del workspace o src-tauri)
    if Path::new("samples").exists() {
        PathBuf::from("samples")
    } else if Path::new("../samples").exists() {
        PathBuf::from("../samples")
    } else {
        PathBuf::from("samples")
    }
}

pub fn scan_soundfonts() -> Vec<SoundFontInfo> {
    let mut list = Vec::new();

    // 1. Escanear SoundFonts de Fábrica
    let factory_dir = get_soundfonts_dir();
    if let Ok(entries) = std::fs::read_dir(&factory_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "sf2") {
                let filename = path.file_name().unwrap().to_string_lossy().to_string();
                let file_size_mb = match entry.metadata() {
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
                } else if filename.to_lowercase().contains("rx7") {
                    (
                        "Yamaha RX7 Drum Machine".to_string(),
                        "Caja de ritmos digital PCM vintage de 1987. Baterías y percusión icónica ochentera.".to_string(),
                        "Drum Machine / Rhythm".to_string(),
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
                    is_user: false,
                });
            }
        }
    }

    // Ordenar de fábrica: Piano, Rhodes, Mellotron, RX7
    list.sort_by(|a, b| {
        let order = |name: &str| {
            if name.contains("Piano") && !name.contains("Rhodes") {
                0
            } else if name.contains("Rhodes") {
                1
            } else if name.contains("Mellotron") {
                2
            } else if name.contains("RX7") {
                3
            } else {
                4
            }
        };
        order(&a.display_name).cmp(&order(&b.display_name))
    });

    // 2. Escanear SoundFonts de Usuario
    let user_dir = get_user_soundfonts_dir();
    let mut user_list = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&user_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "sf2") {
                let filename = path.file_name().unwrap().to_string_lossy().to_string();
                let file_size_mb = match entry.metadata() {
                    Ok(m) => (m.len() as f32) / (1024.0 * 1024.0),
                    Err(_) => 0.0,
                };

                let presets = if let Ok(mut file) = File::open(&path) {
                    if let Ok(sf) = SoundFont::new(&mut file) {
                        sf.get_presets()
                            .iter()
                            .enumerate()
                            .map(|(idx, p)| SoundFontPresetInfo {
                                index: idx,
                                name: p.get_name().to_string(),
                                bank: p.get_bank_number() as i32,
                                patch: p.get_patch_number() as i32,
                            })
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

                let display_name = if let Some(first_p) = presets.first() {
                    if !first_p.name.trim().is_empty() && first_p.name != "Unnamed" {
                        first_p.name.clone()
                    } else {
                        filename.replace(".sf2", "")
                    }
                } else {
                    filename.replace(".sf2", "")
                };

                let description = if presets.is_empty() {
                    "Instrumento SoundFont importado por el usuario.".to_string()
                } else {
                    format!("SoundFont de usuario ({} presets disponibles).", presets.len())
                };

                let id = format!("user-{}", filename.to_lowercase().replace(' ', "-").replace(".sf2", ""));

                user_list.push(SoundFontInfo {
                    id,
                    filename,
                    display_name,
                    description,
                    category: "User SF2".to_string(),
                    file_size_mb: (file_size_mb * 10.0).round() / 10.0,
                    presets,
                    is_user: true,
                });
            }
        }
    }

    user_list.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    list.extend(user_list);
    list
}

pub fn load_soundfont_from_file(filename: &str) -> Result<(Arc<SoundFont>, SoundFontInfo), String> {
    let factory_dir = get_soundfonts_dir();
    let user_dir = get_user_soundfonts_dir();

    let file_path = if factory_dir.join(filename).exists() {
        factory_dir.join(filename)
    } else if user_dir.join(filename).exists() {
        user_dir.join(filename)
    } else if Path::new(filename).exists() {
        PathBuf::from(filename)
    } else {
        return Err(format!("Archivo no encontrado: {:?}", filename));
    };

    let is_user = file_path.starts_with(&user_dir) || !factory_dir.join(filename).exists();

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

    let (display_name, description, category) = if is_user {
        let name = if let Some(first_p) = presets.first() {
            if !first_p.name.trim().is_empty() && first_p.name != "Unnamed" {
                first_p.name.clone()
            } else {
                filename.replace(".sf2", "")
            }
        } else {
            filename.replace(".sf2", "")
        };
        let desc = format!("SoundFont de usuario ({} presets disponibles).", presets.len());
        (name, desc, "User SF2".to_string())
    } else if filename.to_lowercase().contains("piano") && !filename.to_lowercase().contains("rhodes") {
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
    } else if filename.to_lowercase().contains("rx7") {
        (
            "Yamaha RX7 Drum Machine".to_string(),
            "Caja de ritmos digital PCM vintage de 1987. Baterías y percusión icónica ochentera.".to_string(),
            "Drum Machine / Rhythm".to_string(),
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
        is_user,
    };

    Ok((sf_arc, info))
}

pub fn import_soundfont_file(source_path: &Path) -> Result<SoundFontInfo, String> {
    if !source_path.exists() {
        return Err("El archivo de origen no existe.".to_string());
    }

    // Validar con rustysynth que sea un SF2 legible
    let presets: Vec<SoundFontPresetInfo> = {
        let mut file = File::open(source_path)
            .map_err(|e| format!("Error abriendo archivo: {}", e))?;
        let sf = SoundFont::new(&mut file)
            .map_err(|e| format!("El archivo no es un SoundFont 2 válido o está dañado: {:?}", e))?;
        sf.get_presets()
            .iter()
            .enumerate()
            .map(|(idx, p)| SoundFontPresetInfo {
                index: idx,
                name: p.get_name().to_string(),
                bank: p.get_bank_number() as i32,
                patch: p.get_patch_number() as i32,
            })
            .collect()
    };

    let user_dir = get_user_soundfonts_dir();
    let filename = source_path
        .file_name()
        .ok_or_else(|| "Nombre de archivo inválido.".to_string())?
        .to_string_lossy()
        .to_string();

    let target_path = user_dir.join(&filename);

    if source_path != target_path {
        std::fs::copy(source_path, &target_path)
            .map_err(|e| format!("Error copiando SoundFont a la biblioteca de usuario: {}", e))?;
    }

    let file_size_mb = match std::fs::metadata(&target_path) {
        Ok(m) => (m.len() as f32) / (1024.0 * 1024.0),
        Err(_) => 0.0,
    };

    let display_name = if let Some(first_p) = presets.first() {
        if !first_p.name.trim().is_empty() && first_p.name != "Unnamed" {
            first_p.name.clone()
        } else {
            filename.replace(".sf2", "")
        }
    } else {
        filename.replace(".sf2", "")
    };

    let description = format!("SoundFont de usuario ({} presets disponibles).", presets.len());
    let id = format!("user-{}", filename.to_lowercase().replace(' ', "-").replace(".sf2", ""));

    Ok(SoundFontInfo {
        id,
        filename,
        display_name,
        description,
        category: "User SF2".to_string(),
        file_size_mb: (file_size_mb * 10.0).round() / 10.0,
        presets,
        is_user: true,
    })
}

pub fn delete_user_soundfont(filename: &str) -> Result<(), String> {
    let user_dir = get_user_soundfonts_dir();
    let target = user_dir.join(filename);
    if target.exists() {
        std::fs::remove_file(&target)
            .map_err(|e| format!("Error eliminando SoundFont: {}", e))?;
        Ok(())
    } else {
        Err("El archivo no se encuentra en la biblioteca de usuario.".to_string())
    }
}
