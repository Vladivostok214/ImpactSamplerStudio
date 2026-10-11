import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Header, AudioStatusInfo } from "./components/Header";
import { SubHeaderToolbar } from "./components/SubHeaderToolbar";
import { HardwareSkeleton } from "./components/HardwareSkeleton";
import { AudioSettingsModal } from "./components/AudioSettingsModal";
import { InstrumentSelectModal } from "./components/InstrumentSelectModal";
import { StepSequencer } from "./components/StepSequencer";
import { GiantMetronomeOverlay } from "./components/GiantMetronomeOverlay";
import { useMidiStream } from "./hooks/useMidiStream";
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
  PatternBank,
} from "./types/sampler";

export default function App() {
  const {
    events,
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
  const [fxRouting, setFxRouting] = useState<FxRouting>("both");

  // Estados del Secuenciador Rítmico (Fase 5 - 16 Pasos, Bancos A/B y Clic Hardware)
  const [sequencerState, setSequencerState] = useState<SequencerStateInfo>({
    isPlaying: false,
    isRecording: false,
    bpm: 120,
    currentStep: 0,
    resolution: "eighth",
    currentBank: "a",
    isClickEnabled: false,
    isSwingEnabled: false,
    selectedPad: 0,
    pattern: Array.from({ length: 8 }, () => Array(16).fill(0)),
  });

  const [isVisualMetronomeVisible, setIsVisualMetronomeVisible] = useState<boolean>(true);
  const [workstationMode, setWorkstationMode] = useState<"keys" | "seq">("keys");

  const [currentBeat, setCurrentBeat] = useState<{ beat: number; maxBeats: number }>({
    beat: 1,
    maxBeats: 8,
  });

  const [audioStatus, setAudioStatus] = useState<AudioStatusInfo | null>(null);

  useEffect(() => {
    invoke<AudioStatusInfo>("get_audio_status")
      .then(setAudioStatus)
      .catch(console.error);

    const unlistenAudio = listen("audio-status-changed", () => {
      invoke<AudioStatusInfo>("get_audio_status")
        .then(setAudioStatus)
        .catch(console.error);
    });

    return () => {
      unlistenAudio.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    // 1. Obtener lista de soundfonts disponibles
    invoke<SoundFontInfo[]>("list_soundfonts")
      .then((sfs) => {
        // Filtrar soundfonts melódicos (Piano, Rhodes, Mellotron) para el selector de teclado
        const melodicOnly = sfs.filter(
          (s) => !s.filename.toLowerCase().includes("rx7") && !s.filename.toLowerCase().includes("drum")
        );
        setAvailableSoundfonts(melodicOnly.length > 0 ? melodicOnly : sfs);
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
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  const handleSelectSoundfont = async (filename: string) => {
    try {
      const info = await invoke<SoundFontInfo>("load_soundfont", { filename });
      setCurrentSoundfont(info);
      setActivePresetIndex(0);
      setHasSelectedInitialInstrument(true);
    } catch (err) {
      console.error("Error al cargar SoundFont:", err);
      throw err;
    }
  };

  const handleSelectPreset = async (patch: number, index: number) => {
    try {
      await invoke("select_soundfont_preset", { patchNumber: patch });
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

            if (padEl && payload.paths.length > 0) {
              const slot = parseInt(padEl.getAttribute("data-pad-slot") || "-1", 10);
              const path = payload.paths[0];
              if (slot >= 0 && slot < 8 && path.toLowerCase().endsWith(".wav")) {
                handleDropFileOnPad(slot, path);
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
        pattern: Array.from({ length: 8 }, () => Array(16).fill(0)),
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

  const handleSequencerToggleSwing = async () => {
    try {
      const isSwingEnabled = await invoke<boolean>("sequencer_toggle_swing");
      setSequencerState((prev) => ({ ...prev, isSwingEnabled }));
    } catch (err) {
      console.error("Error al alternar swing:", err);
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

  const activePortName =
    ports.find((p) => p.isConnected)?.name ||
    (connected ? "Impact LX49+" : "");

  return (
    <div className="h-screen w-screen flex flex-col bg-black text-slate-100 select-none overflow-hidden relative">
      {/* Overlay Radical de Números Gigantes sincronizados con el Reloj Rust (Fase 5) */}
      <GiantMetronomeOverlay
        isPlaying={sequencerState.isPlaying}
        isVisible={isVisualMetronomeVisible}
        beat={currentBeat.beat}
        maxBeats={currentBeat.maxBeats}
        resolution={sequencerState.resolution}
      />

      {/* Header Minimalista (Logo + Monitor MIDI + Ajustes) */}
      <Header
        onOpenSettings={() => setIsSettingsOpen(true)}
        midiEvents={events}
      />

      {/* Barra de Monitoreo & Control Superior Glassmorphic */}
      <SubHeaderToolbar
        connected={connected}
        activePortName={activePortName}
        audioStatus={audioStatus}
        masterGain={masterGain}
        onMasterGainChange={handleMasterGainChange}
      />

      {/* Contenido Principal: Hardware Monolítico Flotante y Centrado Ergonómicamente sobre Negro Profundo */}
      <main className="flex-1 p-2 sm:p-4 flex flex-col justify-center items-center overflow-y-auto overflow-x-hidden bg-[#000000] relative">
        <div className="studio-console flex flex-col w-full max-w-6xl my-auto relative z-10 transition-all duration-200">
          {/* Consola de Estudio Monolítica Unificada (Controlador Hardware + Docking Multimodal) */}
          <HardwareSkeleton
            activeNotes={activeNotes}
            faders={faders}
            knobs={knobs}
            pitchBend={pitchBend}
            modWheel={modWheel}
            keyRange={
              currentSoundfont?.filename.toLowerCase().includes("mellotron")
                ? { minNote: 43, maxNote: 77, label: "G2 a F5" }
                : undefined
            }
            padSlots={padSlots}
            knobMappings={knobMappings}
            onSetKnobMapping={handleSetKnobMapping}
            fxRouting={fxRouting}
            onChangeFxRouting={handleSetFxRouting}
            currentSoundfont={currentSoundfont}
            activePresetIndex={activePresetIndex}
            onOpenInstrumentSelect={() => setIsInstrumentModalOpen(true)}
            onSelectPreset={handleSelectPreset}
            onTriggerNote={(note, velocity) => triggerVirtualNote(note, velocity, true)}
            onReleaseNote={(note) => triggerVirtualNote(note, 0, false)}
            onChangeFader={triggerVirtualFader}
            onChangeKnob={triggerVirtualKnob}
            onChangePitchBend={triggerVirtualPitchBend}
            onChangeModWheel={triggerVirtualModWheel}
            hideKeyboard={workstationMode === "seq"}
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
              onToggleSwing={handleSequencerToggleSwing}
              onClearSelectedTrack={handleSequencerClearSelectedTrack}
              onClearTrack={handleSequencerClearSelectedTrack}
              onSelectPad={handleSequencerSelectPad}
              isVisualMetronomeVisible={isVisualMetronomeVisible}
              onToggleVisualMetronome={handleToggleVisualMetronome}
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
        onClose={() => setIsInstrumentModalOpen(false)}
      />

      {/* Modal de Configuración */}
      <AudioSettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        ports={ports}
        onRefreshMidi={refreshPorts}
        onSelectPort={selectPort}
      />
    </div>
  );
}
