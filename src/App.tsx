import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Header, AudioStatusInfo } from "./components/Header";
import { HardwareSkeleton } from "./components/HardwareSkeleton";
import { AudioSettingsModal } from "./components/AudioSettingsModal";
import { InstrumentSelectModal } from "./components/InstrumentSelectModal";
import { StepSequencer } from "./components/StepSequencer";
import { GiantMetronomeOverlay } from "./components/GiantMetronomeOverlay";
import { useMidiStream } from "./hooks/useMidiStream";
import { useQwertyKeyboard } from "./hooks/useQwertyKeyboard";
import { SoundFontInfo } from "./types/soundfont";
import {
  PadSlotInfo,
  PadEngineMode,
  KnobMappingInfo,
  KnobTarget,
  FxRouting,
  SequencerStateInfo,
  SequencerStepEvent,
  StepResolution,
  QuantizeMode,
  PatternBank,
} from "./types/sampler";

export default function App() {
  const {
    events,
    clearEvents,
    ports,
    activeNotes,
    activePads,
    faders,
    knobs,
    pitchBend,
    modWheel,
    connected,
    refreshPorts,
    selectPort,
    triggerVirtualNote,
    triggerVirtualPad,
    triggerVirtualFader,
    triggerVirtualKnob,
    triggerVirtualPitchBend,
    triggerVirtualModWheel,
    isMidiLearnActive,
    toggleMidiLearn,
    selectedLearnTarget,
    setSelectedLearnTarget,
    midiLearnMapping,
    resetMidiMappings,
  } = useMidiStream();

  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [masterGain, setMasterGain] = useState(0.8);

  // Estados de Instrumentos SoundFont (Fase 2)
  const [availableSoundfonts, setAvailableSoundfonts] = useState<SoundFontInfo[]>([]);
  const [currentSoundfont, setCurrentSoundfont] = useState<SoundFontInfo | null>(null);
  const [activePresetIndex, setActivePresetIndex] = useState<number>(0);
  const [isInstrumentModalOpen, setIsInstrumentModalOpen] = useState<boolean>(true);
  const [hasSelectedInitialInstrument, setHasSelectedInitialInstrument] = useState<boolean>(false);

  // Estados de Pads Sampler PCM & Drum Kit (Fase 3)
  const [padSlots, setPadSlots] = useState<PadSlotInfo[]>([]);
  const [padEngineMode, setPadEngineMode] = useState<PadEngineMode>("wavSampler");
  const [hoveredDropPad, setHoveredDropPad] = useState<number | null>(null);

  // Estados de Knobs Matrix y FX Rack (Fase 4)
  const [knobMappings, setKnobMappings] = useState<KnobMappingInfo[]>([]);
  const [fxRouting, setFxRouting] = useState<FxRouting>("off");

  // Estados del Secuenciador Rítmico (Fase 5 - 16 Pasos, Bancos A/B y Clic Hardware)
  const [sequencerState, setSequencerState] = useState<SequencerStateInfo>({
    isPlaying: false,
    isPaused: false,
    isRecording: false,
    bpm: 120,
    currentStep: 0,
    resolution: "sixteenth",
    currentBank: "a",
    isClickEnabled: false,
    selectedPad: 0,
    quantizeMode: "sixteenth",
    quantizeStrength: 1.0,
    latencyCompensationTicks: 0,
    pattern: Array.from({ length: 8 }, () => Array(32).fill(0)),
  });

  const [isVisualMetronomeVisible, setIsVisualMetronomeVisible] = useState<boolean>(false);
  const [workstationMode, setWorkstationMode] = useState<"keys" | "seq">("keys");
  const [qwertyEnabled, setQwertyEnabled] = useState<boolean>(false);

  // Rango acústico para instrumentos limitados (ej. Mellotron 43 a 77: G2 a F5)
  const isMellotron = currentSoundfont?.filename.toLowerCase().includes("mellotron");
  const keyRange = isMellotron
    ? { minNote: 43, maxNote: 77, label: "G2 a F5" }
    : undefined;

  // Entrada por teclado de computadora (QWERTY)
  const { octaveOffset } = useQwertyKeyboard({
    enabled: qwertyEnabled,
    workstationMode,
    keyRange,
    onNoteOn: (note, vel) => triggerVirtualNote(note, vel, true),
    onNoteOff: (note) => triggerVirtualNote(note, 0, false),
    onPadOn: (padIdx) => triggerVirtualPad(padIdx, 127, true),
    onPadOff: (padIdx) => triggerVirtualPad(padIdx, 0, false),
  });

  // Splash inicial de 3 segundos con revelado cinematográfico
  const [isBooting, setIsBooting] = useState<boolean>(true);
  const [bootFadeOut, setBootFadeOut] = useState<boolean>(false);

  useEffect(() => {
    // Revelar la ventana inmediatamente una vez montado el splash negro, evitando cualquier destello gris de WebView2
    invoke("show_main_window").catch(() => {});

    // 3 segundos exactos para apreciar el logotipo de IMPACT SAMPLER
    const timer = setTimeout(() => {
      setBootFadeOut(true);
      setTimeout(() => {
        setIsBooting(false);
      }, 700);
    }, 3000);
    return () => clearTimeout(timer);
  }, []);

  const [currentBeat, setCurrentBeat] = useState<{ beat: number; maxBeats: number }>({
    beat: 1,
    maxBeats: 8,
  });

  const [audioStatus, setAudioStatus] = useState<AudioStatusInfo | null>(null);

  useEffect(() => {
    invoke<AudioStatusInfo>("get_audio_status")
      .then((status) => {
        setAudioStatus(status);
        const devName = status?.deviceName || status?.device_name;
        if (devName) {
          try {
            const saved = JSON.parse(localStorage.getItem("impact_sampler_latency_by_device") || "{}");
            if (saved[devName] !== undefined) {
              const ticks = saved[devName];
              invoke("sequencer_set_latency_compensation", { ticks }).catch(console.error);
              setSequencerState((prev) => ({ ...prev, latencyCompensationTicks: ticks }));
            }
          } catch (_) {}
        }
      })
      .catch(console.error);

    const unlistenAudio = listen("audio-status-changed", () => {
      invoke<AudioStatusInfo>("get_audio_status")
        .then((status) => {
          setAudioStatus(status);
          const devName = status?.deviceName || status?.device_name;
          if (devName) {
            try {
              const saved = JSON.parse(localStorage.getItem("impact_sampler_latency_by_device") || "{}");
              if (saved[devName] !== undefined) {
                const ticks = saved[devName];
                invoke("sequencer_set_latency_compensation", { ticks }).catch(console.error);
                setSequencerState((prev) => ({ ...prev, latencyCompensationTicks: ticks }));
              }
            } catch (_) {}
          }
        })
        .catch(console.error);
    });

    return () => {
      unlistenAudio.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    // 1. Obtener lista de soundfonts disponibles (Piano, Rhodes, Mellotron, Yamaha RX7)
    invoke<SoundFontInfo[]>("list_soundfonts")
      .then((sfs) => {
        const order = ["piano", "rhodes", "mellotron", "rx7"];
        const sorted = [...sfs].sort((a, b) => {
          const idxA = order.findIndex((k) => a.filename.toLowerCase().includes(k));
          const idxB = order.findIndex((k) => b.filename.toLowerCase().includes(k));
          return (idxA === -1 ? 99 : idxA) - (idxB === -1 ? 99 : idxB);
        });
        setAvailableSoundfonts(sorted.length > 0 ? sorted : sfs);
      })
      .catch(console.error);

    // 2. Verificar si ya hay soundfont activo
    invoke<SoundFontInfo | null>("get_current_soundfont")
      .then((cur) => {
        if (cur) {
          setCurrentSoundfont(cur);
          setHasSelectedInitialInstrument(true);
          setIsInstrumentModalOpen(false);
        } else {
          setIsInstrumentModalOpen(true);
        }
      })
      .catch(console.error);

    // 3. Inicializar slots de pads y modo rítmico (Fase 3)
    invoke<PadSlotInfo[]>("get_pad_slots")
      .then((slots) => setPadSlots(slots))
      .catch(console.error);

    invoke<PadEngineMode>("get_pad_engine_mode")
      .then((mode) => setPadEngineMode(mode))
      .catch(console.error);

    // 4. Inicializar mapeo de knobs y FX Rack (Fase 4)
    invoke<KnobMappingInfo[]>("get_knob_mappings")
      .then((mappings) => setKnobMappings(mappings))
      .catch(console.error);

    invoke<FxRouting>("get_fx_routing")
      .then((routing) => setFxRouting(routing))
      .catch(console.error);

    // 5. Inicializar y suscribirse al Secuenciador Rítmico (Fase 5)
    invoke<SequencerStateInfo>("get_sequencer_state")
      .then((state) => setSequencerState(state))
      .catch(console.error);

    let unlistenStep: (() => void) | undefined;
    let unlistenState: (() => void) | undefined;
    let unlistenPattern: (() => void) | undefined;
    let unlistenToggleVisualMetronome: (() => void) | undefined;
    let unlistenAdvancePage: (() => void) | undefined;
    let unlistenToggleWorkstation: (() => void) | undefined;
    let unlistenFxRouting: (() => void) | undefined;

    listen<SequencerStepEvent>("sequencer-step", (event) => {
      setSequencerState((prev) => ({
        ...prev,
        currentStep: event.payload.step,
      }));
      setCurrentBeat({
        beat: event.payload.beat,
        maxBeats: event.payload.maxBeats,
      });
    }).then((un) => {
      unlistenStep = un;
    });

    listen<SequencerStateInfo>("sequencer-state-changed", (event) => {
      setSequencerState(event.payload);
    }).then((un) => {
      unlistenState = un;
    });

    listen<number[][]>("sequencer-pattern-updated", (event) => {
      setSequencerState((prev) => ({
        ...prev,
        pattern: event.payload,
      }));
    }).then((un) => {
      unlistenPattern = un;
    });

    listen("toggle-visual-metronome", () => {
      setIsVisualMetronomeVisible((prev) => !prev);
    }).then((un) => {
      unlistenToggleVisualMetronome = un;
    });

    listen("sequencer-advance-page", () => {
      // Disparar evento personalizado de avance de página
      window.dispatchEvent(new CustomEvent("midi-advance-page"));
    }).then((un) => {
      unlistenAdvancePage = un;
    });

    listen("toggle-workstation-mode", () => {
      setWorkstationMode((prev) => (prev === "keys" ? "seq" : "keys"));
    }).then((un) => {
      unlistenToggleWorkstation = un;
    });

    listen<FxRouting>("fx-routing-changed", (event) => {
      setFxRouting(event.payload);
    }).then((un) => {
      unlistenFxRouting = un;
    });

    // Atajo de teclado: Barra espaciadora para Play/Pause
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        e.code === "Space" &&
        !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)
      ) {
        e.preventDefault();
        invoke("sequencer_toggle_play").catch(console.error);
      }
    };
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      if (unlistenStep) unlistenStep();
      if (unlistenState) unlistenState();
      if (unlistenPattern) unlistenPattern();
      if (unlistenToggleVisualMetronome) unlistenToggleVisualMetronome();
      if (unlistenAdvancePage) unlistenAdvancePage();
      if (unlistenToggleWorkstation) unlistenToggleWorkstation();
      if (unlistenFxRouting) unlistenFxRouting();
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  const refreshSoundfonts = async () => {
    try {
      const sfs = await invoke<SoundFontInfo[]>("list_soundfonts");
      const order = ["piano", "rhodes", "mellotron", "rx7"];
      const sorted = [...sfs].sort((a, b) => {
        if (!a.isUser && !b.isUser) {
          const idxA = order.findIndex((k) => a.filename.toLowerCase().includes(k));
          const idxB = order.findIndex((k) => b.filename.toLowerCase().includes(k));
          return (idxA === -1 ? 99 : idxA) - (idxB === -1 ? 99 : idxB);
        }
        if (!a.isUser && b.isUser) return -1;
        if (a.isUser && !b.isUser) return 1;
        return a.displayName.localeCompare(b.displayName);
      });
      setAvailableSoundfonts(sorted.length > 0 ? sorted : sfs);
    } catch (err) {
      console.error("Error al actualizar lista de soundfonts:", err);
    }
  };

  const handleSelectSoundfont = async (filename: string) => {
    try {
      const info = await invoke<SoundFontInfo>("load_soundfont", { filename });
      setCurrentSoundfont(info);
      setActivePresetIndex(0);
      setHasSelectedInitialInstrument(true);

      // Si el instrumento seleccionado es Yamaha RX7, abrir directamente en modo secuenciador y activar pads de batería
      if (filename.toLowerCase().includes("rx7")) {
        setWorkstationMode("seq");
        await handleChangePadEngineMode("drumKitSf2");
      } else {
        setWorkstationMode("keys");
      }

      setIsInstrumentModalOpen(false);
      (document.activeElement as HTMLElement)?.blur?.();
    } catch (err) {
      console.error("Error al cargar SoundFont:", err);
      throw err;
    }
  };

  const handleImportSoundfont = async () => {
    try {
      const imported = await invoke<SoundFontInfo | null>("pick_and_import_soundfont");
      if (imported) {
        await refreshSoundfonts();
        await handleSelectSoundfont(imported.filename);
      }
    } catch (err) {
      console.error("Error al importar SoundFont:", err);
    }
  };

  const handleDeleteSoundfont = async (filename: string) => {
    try {
      await invoke("remove_user_soundfont", { filename });
      await refreshSoundfonts();
    } catch (err) {
      console.error("Error al eliminar SoundFont:", err);
    }
  };

  const handleSelectPreset = async (patch: number, index: number, bank?: number) => {
    try {
      const bankNumber = bank ?? (currentSoundfont?.presets[index]?.bank ?? 0);
      await invoke("select_soundfont_preset", { patchNumber: patch, bankNumber });
      setActivePresetIndex(index);
    } catch (err) {
      console.error("Error al seleccionar preset:", err);
    }
  };

  // Handlers para Pads (Fase 3)
  const handlePickPadSample = async (padIdx: number) => {
    try {
      const updated = await invoke<PadSlotInfo | null>("pick_and_load_pad_sample", {
        padIndex: padIdx,
      });
      if (updated) {
        setPadSlots((prev) => {
          const next = [...prev];
          next[padIdx] = updated;
          return next;
        });
      }
    } catch (err) {
      console.error("Error al seleccionar muestra WAV:", err);
    }
  };

  const handleClearPadSlot = async (padIdx: number) => {
    try {
      const updated = await invoke<PadSlotInfo>("clear_pad_slot", {
        padIndex: padIdx,
      });
      setPadSlots((prev) => {
        const next = [...prev];
        next[padIdx] = updated;
        return next;
      });
    } catch (err) {
      console.error("Error al vaciar slot de pad:", err);
    }
  };

  const handleTogglePadMode = async (padIdx: number) => {
    const current = padSlots[padIdx];
    const newMode = current?.mode === "gate" ? "oneShot" : "gate";
    try {
      const updated = await invoke<PadSlotInfo>("set_pad_slot_mode", {
        padIndex: padIdx,
        mode: newMode,
      });
      setPadSlots((prev) => {
        const next = [...prev];
        next[padIdx] = updated;
        return next;
      });
    } catch (err) {
      console.error("Error al alternar modo del pad:", err);
    }
  };

  const handleChangePadEngineMode = async (mode: PadEngineMode) => {
    try {
      await invoke("set_pad_engine_mode", { mode });
      setPadEngineMode(mode);
    } catch (err) {
      console.error("Error al cambiar modo de pads:", err);
    }
  };

  const handleDropFileOnPad = async (padIdx: number, filePath: string) => {
    try {
      const updated = await invoke<PadSlotInfo>("load_pad_sample_from_file", {
        padIndex: padIdx,
        path: filePath,
      });
      setPadSlots((prev) => {
        const next = [...prev];
        next[padIdx] = updated;
        return next;
      });
    } catch (err) {
      console.error("Error al cargar muestra arrastrada:", err);
    }
  };

  const handleSetKnobMapping = async (knobIndex: number, target: KnobTarget) => {
    try {
      const updated = await invoke<KnobMappingInfo[]>("set_knob_mapping", {
        knobIndex,
        target,
      });
      setKnobMappings(updated);
    } catch (err) {
      console.error("Error al reasignar knob:", err);
    }
  };

  const handleSetFxRouting = async (routing: FxRouting) => {
    try {
      const updated = await invoke<FxRouting>("set_fx_routing", { routing });
      setFxRouting(updated);
    } catch (err) {
      console.error("Error al cambiar ruteo FX:", err);
    }
  };

  // Listener nativo de Drag & Drop en Tauri v2 desde el Explorador de Windows
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let isMounted = true;

    const setupDragDrop = async () => {
      try {
        const appWindow = getCurrentWebviewWindow();
        const unlistenFn = await appWindow.onDragDropEvent((event) => {
          const payload = event.payload;

          if (payload.type === "over" || payload.type === "enter") {
            const scale = window.devicePixelRatio || 1;
            const clientX = payload.position.x / scale;
            const clientY = payload.position.y / scale;

            const el = document.elementFromPoint(clientX, clientY);
            const padEl = el?.closest("[data-pad-slot]");
            if (padEl) {
              const slot = parseInt(padEl.getAttribute("data-pad-slot") || "-1", 10);
              if (slot >= 0 && slot < 8) {
                setHoveredDropPad(slot);
                return;
              }
            }
            setHoveredDropPad(null);
          } else if (payload.type === "drop") {
            const scale = window.devicePixelRatio || 1;
            const clientX = payload.position.x / scale;
            const clientY = payload.position.y / scale;

            const el = document.elementFromPoint(clientX, clientY);
            const padEl = el?.closest("[data-pad-slot]");
            setHoveredDropPad(null);

            if (payload.paths.length > 0) {
              const path = payload.paths[0];
              // Si se arrastra un archivo .sf2 en cualquier parte de la ventana
              if (path.toLowerCase().endsWith(".sf2")) {
                invoke<SoundFontInfo>("import_soundfont_from_path", { filePath: path })
                  .then((imported) => {
                    refreshSoundfonts();
                    handleSelectSoundfont(imported.filename);
                  })
                  .catch((err) => {
                    console.error("Error al importar soundfont arrastrado:", err);
                  });
                return;
              }

              // Si se arrastra un .wav sobre un slot de pad
              if (padEl) {
                const slot = parseInt(padEl.getAttribute("data-pad-slot") || "-1", 10);
                if (slot >= 0 && slot < 8 && path.toLowerCase().endsWith(".wav")) {
                  handleDropFileOnPad(slot, path);
                }
              }
            }
          } else if (payload.type === "leave") {
            setHoveredDropPad(null);
          }
        });

        if (isMounted) {
          unlisten = unlistenFn;
        } else {
          unlistenFn();
        }
      } catch (err) {
        console.error("Error al configurar listener de Drag and Drop nativo:", err);
      }
    };

    setupDragDrop();

    return () => {
      isMounted = false;
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    if (faders[8] !== undefined) {
      setMasterGain(faders[8] / 127);
    }
  }, [faders[8]]);

  const handleMasterGainChange = (gain: number) => {
    setMasterGain(gain);
    triggerVirtualFader(8, Math.round(gain * 127));
  };

  // Handlers para el Secuenciador Rítmico (Fase 5)
  const handleSequencerTogglePlay = async () => {
    try {
      const isPlaying = await invoke<boolean>("sequencer_toggle_play");
      setSequencerState((prev) => ({ ...prev, isPlaying }));
    } catch (err) {
      console.error("Error al alternar play:", err);
    }
  };

  const handleSequencerStop = async () => {
    try {
      await invoke("sequencer_stop");
      setSequencerState((prev) => ({ ...prev, isPlaying: false, currentStep: 0 }));
    } catch (err) {
      console.error("Error al detener secuenciador:", err);
    }
  };

  const handleSequencerToggleRecord = async () => {
    try {
      const isRecording = await invoke<boolean>("sequencer_toggle_record");
      setSequencerState((prev) => ({ ...prev, isRecording }));
    } catch (err) {
      console.error("Error al alternar overdub record:", err);
    }
  };

  const handleSequencerSetBpm = async (bpm: number) => {
    try {
      await invoke("sequencer_set_bpm", { bpm });
      setSequencerState((prev) => ({ ...prev, bpm }));
    } catch (err) {
      console.error("Error al cambiar BPM:", err);
    }
  };

  const handleSequencerAdjustBpm = async (delta: number) => {
    try {
      const newBpm = await invoke<number>("sequencer_adjust_bpm", { delta });
      setSequencerState((prev) => ({ ...prev, bpm: newBpm }));
    } catch (err) {
      console.error("Error al ajustar BPM:", err);
    }
  };

  const handleSequencerTapTempo = async () => {
    try {
      const newBpm = await invoke<number | null>("sequencer_tap_tempo");
      if (newBpm !== null) {
        setSequencerState((prev) => ({ ...prev, bpm: newBpm }));
      }
    } catch (err) {
      console.error("Error al calcular Tap Tempo:", err);
    }
  };

  const handleSequencerToggleStep = async (padIdx: number, stepIdx: number) => {
    try {
      const newVal = await invoke<number>("sequencer_toggle_step", {
        padIndex: padIdx,
        stepIndex: stepIdx,
      });
      setSequencerState((prev) => {
        const nextPat = prev.pattern.map((row) => [...row]);
        nextPat[padIdx][stepIdx] = newVal;
        return { ...prev, pattern: nextPat };
      });
    } catch (err) {
      console.error("Error al alternar paso:", err);
    }
  };

  const handleSequencerClear = async () => {
    try {
      await invoke("sequencer_clear");
      setSequencerState((prev) => ({
        ...prev,
        pattern: Array.from({ length: 8 }, () => Array(32).fill(0)),
      }));
    } catch (err) {
      console.error("Error al limpiar secuenciador:", err);
    }
  };

  const handleSequencerChangeResolution = async (resolution: StepResolution) => {
    try {
      await invoke("sequencer_set_resolution", { resolution });
      setSequencerState((prev) => ({ ...prev, resolution }));
      setCurrentBeat((prev) => ({
        ...prev,
        maxBeats: resolution === "thirtySecond" ? 2 : resolution === "sixteenth" ? 4 : 8,
      }));
    } catch (err) {
      console.error("Error al cambiar resolución del secuenciador:", err);
    }
  };

  const handleSequencerToggleClick = async () => {
    try {
      const isClickEnabled = await invoke<boolean>("sequencer_toggle_click");
      setSequencerState((prev) => ({ ...prev, isClickEnabled }));
    } catch (err) {
      console.error("Error al alternar clic metrónomo:", err);
    }
  };

  const handleSequencerToggleBank = async () => {
    try {
      const res = await invoke<{ currentBank: PatternBank; pattern: number[][] }>(
        "sequencer_toggle_bank"
      );
      setSequencerState((prev) => ({
        ...prev,
        currentBank: res.currentBank,
        pattern: res.pattern,
      }));
    } catch (err) {
      console.error("Error al alternar banco de patrones:", err);
    }
  };

  const handleSequencerSelectPad = async (padIdx: number) => {
    try {
      await invoke("sequencer_set_selected_pad", { padIndex: padIdx });
      setSequencerState((prev) => ({ ...prev, selectedPad: padIdx }));
    } catch (err) {
      console.error("Error al seleccionar pad:", err);
    }
  };

  const handleSequencerClearSelectedTrack = async (targetPadIdx?: number) => {
    const padToClear = targetPadIdx !== undefined ? targetPadIdx : sequencerState.selectedPad;
    try {
      await invoke("sequencer_clear_track", { padIndex: padToClear });
      setSequencerState((prev) => {
        const nextPat = prev.pattern.map((row) => [...row]);
        if (padToClear < 8) {
          nextPat[padToClear].fill(0);
        }
        return { ...prev, pattern: nextPat };
      });
    } catch (err) {
      console.error("Error al limpiar pista:", err);
    }
  };

  const handleToggleVisualMetronome = () => {
    setIsVisualMetronomeVisible((prev) => !prev);
  };

  const handleSequencerTriggerPad = (padIdx: number) => {
    triggerVirtualPad(padIdx + 1, 36 + padIdx, true);
    setTimeout(() => {
      triggerVirtualPad(padIdx + 1, 36 + padIdx, false);
    }, 120);
  };

  const handleSequencerSetQuantizeMode = async (mode: QuantizeMode) => {
    try {
      await invoke("sequencer_set_quantize_mode", { mode });
      setSequencerState((prev) => ({ ...prev, quantizeMode: mode }));
    } catch (err) {
      console.error("Error al cambiar modo de cuantización:", err);
    }
  };

  const handleSequencerSetQuantizeStrength = async (strength: number) => {
    try {
      await invoke("sequencer_set_quantize_strength", { strength });
      setSequencerState((prev) => ({ ...prev, quantizeStrength: strength }));
    } catch (err) {
      console.error("Error al cambiar fuerza de cuantización:", err);
    }
  };

  const handleSequencerRequantize = async () => {
    try {
      await invoke("sequencer_requantize");
    } catch (err) {
      console.error("Error al re-cuantizar patrón:", err);
    }
  };

  const handleSequencerSetLatencyCompensation = async (ticks: number) => {
    try {
      await invoke("sequencer_set_latency_compensation", { ticks });
      setSequencerState((prev) => ({ ...prev, latencyCompensationTicks: ticks }));
    } catch (err) {
      console.error("Error al configurar compensación de latencia:", err);
    }
  };

  const handleSequencerUndo = async () => {
    try {
      await invoke("sequencer_undo");
    } catch (err) {
      console.error("Error al deshacer acción en secuenciador:", err);
    }
  };

  const activePortName =
    ports.find((p) => p.isConnected)?.name ||
    (connected ? "Impact LX49+" : "");

  // Iluminación ambiental reactiva al instrumento seleccionado (con presencia visible y contraste elegante)
  const getAmbientGlow = () => {
    const fn = currentSoundfont?.filename.toLowerCase() || "";
    if (fn.includes("piano") && !fn.includes("rhodes")) {
      return "radial-gradient(ellipse 85% 65% at 50% 45%, rgba(245, 158, 11, 0.20) 0%, rgba(245, 158, 11, 0.07) 45%, transparent 75%)";
    } else if (fn.includes("rhodes")) {
      return "radial-gradient(ellipse 85% 65% at 50% 45%, rgba(6, 182, 212, 0.20) 0%, rgba(6, 182, 212, 0.07) 45%, transparent 75%)";
    } else if (fn.includes("mellotron")) {
      return "radial-gradient(ellipse 85% 65% at 50% 45%, rgba(244, 63, 94, 0.20) 0%, rgba(244, 63, 94, 0.07) 45%, transparent 75%)";
    } else if (fn.includes("rx7")) {
      return "radial-gradient(ellipse 85% 65% at 50% 45%, rgba(168, 85, 247, 0.22) 0%, rgba(168, 85, 247, 0.08) 45%, transparent 75%)";
    }
    return "radial-gradient(ellipse 85% 65% at 50% 45%, rgba(16, 185, 129, 0.16) 0%, rgba(16, 185, 129, 0.05) 45%, transparent 75%)";
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-[#050507] text-slate-100 select-none overflow-hidden relative">
      {/* Overlay Radical de Números Gigantes sincronizados con el Reloj Rust (Fase 5) */}
      <GiantMetronomeOverlay
        isPlaying={sequencerState.isPlaying}
        isVisible={isVisualMetronomeVisible}
        beat={currentBeat.beat}
        maxBeats={currentBeat.maxBeats}
        resolution={sequencerState.resolution}
      />

      {/* Header Unificado de Estudio (Logo + Monitores Telemetría + Monitor MIDI DIN + Ajustes + Master Volume + MIDI Learn) */}
      <Header
        onOpenSettings={() => setIsSettingsOpen(true)}
        onOpenInstrumentSelect={() => setIsInstrumentModalOpen(true)}
        midiEvents={events}
        onClearMidiEvents={clearEvents}
        connected={connected}
        activePortName={activePortName}
        audioStatus={audioStatus}
        masterGain={masterGain}
        onMasterGainChange={handleMasterGainChange}
        isMidiLearnActive={isMidiLearnActive}
        onToggleMidiLearn={toggleMidiLearn}
        qwertyEnabled={qwertyEnabled}
        onToggleQwerty={() => setQwertyEnabled((v) => !v)}
      />

      {/* Contenido Principal: Hardware Monolítico Flotante y Centrado Ergonómicamente sobre Negro Profundo */}
      <main className="flex-1 p-2 sm:p-4 flex flex-col justify-center items-center overflow-y-auto overflow-x-hidden bg-[#070709] relative">
        {/* Capas de Fondo de Estudio Noir (Iluminación Cenital + Micro-Malla + Aura Reactiva) */}
        <div className="absolute inset-0 pointer-events-none z-0 overflow-hidden">
          {/* 1. Iluminación Cenital Definida (Studio Overhead Spotlight) */}
          <div
            className="absolute inset-0"
            style={{
              background: "radial-gradient(ellipse 80% 50% at 50% 0%, rgba(255, 255, 255, 0.12) 0%, rgba(255, 255, 255, 0.03) 50%, transparent 75%)",
            }}
          />

          {/* 2. Micro-Malla Acústica de Estudio (Precision Dot Matrix con alta nitidez) */}
          <div
            className="absolute inset-0"
            style={{
              backgroundImage: "radial-gradient(circle, rgba(255, 255, 255, 0.22) 1.2px, transparent 1.2px)",
              backgroundSize: "28px 28px",
              maskImage: "radial-gradient(ellipse 85% 75% at 50% 50%, black 40%, transparent 95%)",
              WebkitMaskImage: "radial-gradient(ellipse 85% 75% at 50% 50%, black 40%, transparent 95%)",
            }}
          />

          {/* 3. Aura Reactiva al Instrumento Activo con Presencia Notoria y Transición Suave */}
          <div
            className="absolute inset-0 transition-all duration-1000 ease-out"
            style={{
              background: getAmbientGlow(),
            }}
          />

          {/* 4. Viñeta Perimetral Profunda para Enmarcar la Consola */}
          <div
            className="absolute inset-0"
            style={{
              background: "radial-gradient(ellipse 95% 85% at 50% 50%, transparent 65%, rgba(0, 0, 0, 0.85) 100%)",
            }}
          />
        </div>

        <div className="studio-console flex flex-col w-full max-w-6xl my-auto relative z-10 transition-all duration-200">
          {/* Consola de Estudio Monolítica Unificada (Controlador Hardware + Docking Multimodal) */}
          <HardwareSkeleton
            activeNotes={activeNotes}
            faders={faders}
            knobs={knobs}
            pitchBend={pitchBend}
            modWheel={modWheel}
            keyRange={keyRange}
            padSlots={padSlots}
            knobMappings={knobMappings}
            onSetKnobMapping={handleSetKnobMapping}
            fxRouting={fxRouting}
            onChangeFxRouting={handleSetFxRouting}
            availableSoundfonts={availableSoundfonts}
            currentSoundfont={currentSoundfont}
            activePresetIndex={activePresetIndex}
            onSelectSoundfont={handleSelectSoundfont}
            onSelectPreset={handleSelectPreset}
            onTriggerNote={(note, velocity) => triggerVirtualNote(note, velocity, true)}
            onReleaseNote={(note) => triggerVirtualNote(note, 0, false)}
            onChangeFader={triggerVirtualFader}
            onChangeKnob={triggerVirtualKnob}
            onChangePitchBend={triggerVirtualPitchBend}
            onChangeModWheel={triggerVirtualModWheel}
            hideKeyboard={workstationMode === "seq"}
            isMidiLearnActive={isMidiLearnActive}
            selectedLearnTarget={selectedLearnTarget}
            onSelectLearnTarget={setSelectedLearnTarget}
            midiLearnMapping={midiLearnMapping}
            qwertyEnabled={qwertyEnabled}
            qwertyOctaveOffset={octaveOffset}
          >
            {/* Bahía Acoplada de Expansión: Secuenciador Rítmico TR-Style & Barra de Docking Maestra */}
            <StepSequencer
              viewMode={workstationMode}
              onChangeViewMode={setWorkstationMode}
              sequencerState={sequencerState}
              padSlots={padSlots}
              padEngineMode={padEngineMode}
              onChangePadEngineMode={handleChangePadEngineMode}
              activePads={activePads}
              onTriggerPadWithNote={(padIdx, note) => triggerVirtualPad(padIdx, note, true)}
              onReleasePadWithNote={(padIdx, note) => triggerVirtualPad(padIdx, note, false)}
              onPickPadSample={handlePickPadSample}
              onClearPadSlot={handleClearPadSlot}
              onTogglePadMode={handleTogglePadMode}
              onDropFileOnPad={handleDropFileOnPad}
              dragOverSlot={hoveredDropPad}
              onTogglePlay={handleSequencerTogglePlay}
              onStop={handleSequencerStop}
              onToggleRecord={handleSequencerToggleRecord}
              onSetBpm={handleSequencerSetBpm}
              onAdjustBpm={handleSequencerAdjustBpm}
              onTapTempo={handleSequencerTapTempo}
              onToggleStep={handleSequencerToggleStep}
              onClear={handleSequencerClear}
              onChangeResolution={handleSequencerChangeResolution}
              onTriggerPad={handleSequencerTriggerPad}
              onToggleClick={handleSequencerToggleClick}
              onToggleBank={handleSequencerToggleBank}
              onClearSelectedTrack={handleSequencerClearSelectedTrack}
              onClearTrack={handleSequencerClearSelectedTrack}
              onSelectPad={handleSequencerSelectPad}
              isVisualMetronomeVisible={isVisualMetronomeVisible}
              onToggleVisualMetronome={handleToggleVisualMetronome}
              isMidiLearnActive={isMidiLearnActive}
              selectedLearnTarget={selectedLearnTarget}
              onSelectLearnTarget={setSelectedLearnTarget}
              midiLearnMapping={midiLearnMapping}
              onSetQuantizeMode={handleSequencerSetQuantizeMode}
              onSetQuantizeStrength={handleSequencerSetQuantizeStrength}
              onRequantize={handleSequencerRequantize}
              onUndo={handleSequencerUndo}
            />
          </HardwareSkeleton>
        </div>
      </main>

      {/* Selector / Splash de Instrumentos */}
      <InstrumentSelectModal
        isOpen={isInstrumentModalOpen}
        isInitialSplash={!hasSelectedInitialInstrument}
        availableSoundfonts={availableSoundfonts}
        currentSoundfont={currentSoundfont}
        onSelectSoundfont={handleSelectSoundfont}
        onImportSoundfont={handleImportSoundfont}
        onDeleteSoundfont={handleDeleteSoundfont}
        onClose={() => setIsInstrumentModalOpen(false)}
      />

      {/* Modal de Configuración */}
      <AudioSettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        ports={ports}
        onRefreshMidi={refreshPorts}
        onSelectPort={selectPort}
        onResetMidiMappings={resetMidiMappings}
        latencyCompensationTicks={sequencerState.latencyCompensationTicks}
        onSetLatencyCompensation={handleSequencerSetLatencyCompensation}
        activePortName={activePortName}
      />

      {/* Splash Inicial de 3 Segundos Noir con Revelado Cinematográfico */}
      {isBooting && (
        <div
          className={`fixed inset-0 z-[100] bg-black flex items-center justify-center select-none transition-all duration-700 ease-out ${
            bootFadeOut
              ? "opacity-0 pointer-events-none scale-110 blur-sm"
              : "opacity-100 scale-100 blur-0"
          }`}
          style={{ fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif' }}
        >
          <h1
            className="text-3xl sm:text-4xl md:text-5xl font-black tracking-widest text-white flex items-center drop-shadow-[0_0_35px_rgba(255,255,255,0.18)]"
            style={{ fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif' }}
          >
            IMPACT<span className="text-emerald-400 drop-shadow-[0_0_24px_rgba(16,185,129,0.75)]">SAMPLER</span>
          </h1>
        </div>
      )}
    </div>
  );
}
