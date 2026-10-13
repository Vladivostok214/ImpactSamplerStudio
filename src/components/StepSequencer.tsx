import React, { useState, useRef, useEffect } from "react";
import { ChevronDown, ChevronRight, Check, FolderOpen, RotateCcw, Eraser, Trash2, Music } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { PadSlotInfo, PadEngineMode, SequencerStateInfo, StepResolution } from "../types/sampler";
import { MidiLearnTarget, MidiLearnMapping, getMidiNoteName } from "../types/midiLearn";

export interface Sf2DrumSample {
  note: number;
  name: string;
}

export interface Sf2DrumCategory {
  id: string;
  name: string;
  samples: Sf2DrumSample[];
}

export const SF2_DRUM_CATEGORIES: Sf2DrumCategory[] = [
  {
    id: "bombo",
    name: "BOMBO",
    samples: [
      { note: 36, name: "Bass Kick 1" },
      { note: 35, name: "Acoustic Kick" },
      { note: 10, name: "Bass Kick 2" },
      { note: 11, name: "Bass Kick 3" },
      { note: 12, name: "Bass Kick 4" },
      { note: 13, name: "Bass Kick 5" },
      { note: 14, name: "Bass Kick 6" },
      { note: 15, name: "Bass Kick 7" },
      { note: 16, name: "Bass Kick 8" },
      { note: 25, name: "Reverse Kick" },
    ],
  },
  {
    id: "caja",
    name: "CAJA",
    samples: [
      { note: 38, name: "Acoustic Snare" },
      { note: 40, name: "Electric Snare" },
      { note: 37, name: "Sidestick / Rim" },
      { note: 0, name: "Snare 1" },
      { note: 1, name: "Snare 2" },
      { note: 2, name: "Snare 3" },
      { note: 3, name: "Snare 4" },
      { note: 4, name: "Snare 5" },
      { note: 5, name: "Snare 6" },
      { note: 6, name: "Snare 7" },
      { note: 7, name: "Snare 8" },
      { note: 8, name: "Snare 9" },
      { note: 32, name: "Reverse Snare" },
    ],
  },
  {
    id: "hihat",
    name: "HI-HAT",
    samples: [
      { note: 42, name: "Closed Hat" },
      { note: 46, name: "Open Hat" },
      { note: 44, name: "Pedal Hat" },
      { note: 24, name: "Pedal Hat 2" },
    ],
  },
  {
    id: "platillo",
    name: "PLATILLO",
    samples: [
      { note: 49, name: "Crash Cymbal" },
      { note: 51, name: "Ride Cymbal" },
      { note: 53, name: "Ride Bell" },
      { note: 52, name: "Chinese Cymbal" },
      { note: 29, name: "FM Synth Crash" },
      { note: 26, name: "Reverse Cymbal" },
      { note: 27, name: "Reverse Bell Ride" },
    ],
  },
  {
    id: "toms",
    name: "TOMS",
    samples: [
      { note: 50, name: "High Tom 1" },
      { note: 48, name: "High-Mid Tom 1" },
      { note: 47, name: "Low-Mid Tom 1" },
      { note: 45, name: "Low Tom 1" },
      { note: 17, name: "Heavy Tom 1" },
      { note: 19, name: "Heavy Tom 2" },
      { note: 22, name: "High Tom 2" },
      { note: 20, name: "High-Mid Tom 2" },
      { note: 18, name: "Low-Mid Tom 2" },
      { note: 21, name: "Low Tom 2" },
      { note: 23, name: "High Tom 3" },
      { note: 43, name: "High-Mid Tom 3" },
      { note: 41, name: "Low-Mid Tom 3" },
      { note: 73, name: "High Electro Tom" },
      { note: 81, name: "High-Mid Electro Tom" },
      { note: 80, name: "Low-Mid Electro Tom" },
      { note: 74, name: "Low Electro Tom" },
      { note: 33, name: "Reverse Tom" },
      { note: 28, name: "Reverse Electro Tom" },
    ],
  },
  {
    id: "percusion",
    name: "PERCUSION",
    samples: [
      { note: 39, name: "Handclap" },
      { note: 56, name: "Cowbell" },
      { note: 54, name: "Tambourine" },
      { note: 69, name: "Cabasa" },
      { note: 60, name: "Hi Bongo" },
      { note: 61, name: "Low Bongo" },
      { note: 62, name: "Mute Conga" },
      { note: 63, name: "Conga" },
      { note: 64, name: "Low Conga" },
      { note: 65, name: "Hi Timbale" },
      { note: 66, name: "Low Timbale" },
      { note: 67, name: "Hi Agogo" },
      { note: 68, name: "Low Agogo" },
      { note: 75, name: "Claves" },
      { note: 79, name: "Cuica" },
      { note: 85, name: "Castanet" },
      { note: 58, name: "Vibraslap" },
      { note: 71, name: "Short Whistle" },
      { note: 72, name: "Long Whistle" },
      { note: 84, name: "Belltree" },
    ],
  },
  {
    id: "fx",
    name: "FX Y VOCES",
    samples: [
      { note: 59, name: "Get Funky!" },
      { note: 77, name: "Hey!" },
      { note: 76, name: "Ha!" },
      { note: 88, name: "Ow!" },
      { note: 34, name: "Bom" },
      { note: 87, name: "Gunshot" },
      { note: 86, name: "Glass Shatter" },
      { note: 55, name: "Camera Shutter" },
      { note: 57, name: "Shutter" },
      { note: 9, name: "Timpani" },
      { note: 30, name: "FM Synth Drum 1" },
      { note: 31, name: "FM Synth Drum 2" },
      { note: 82, name: "FM 1" },
      { note: 83, name: "FM 2" },
    ],
  },
];

interface StepSequencerProps {
  viewMode?: "keys" | "seq";
  onChangeViewMode?: (mode: "keys" | "seq") => void;
  sequencerState: SequencerStateInfo;
  padSlots: PadSlotInfo[];
  padEngineMode: PadEngineMode;
  onChangePadEngineMode?: (mode: PadEngineMode) => void;
  activePads?: Map<number, number>;
  onTriggerPadWithNote?: (padIdx: number, note: number) => void;
  onReleasePadWithNote?: (padIdx: number, note: number) => void;
  onPickPadSample?: (padIdx: number) => void;
  onClearPadSlot?: (padIdx: number) => void;
  onTogglePadMode?: (padIdx: number) => void;
  onDropFileOnPad?: (padIdx: number, filePath: string) => void;
  dragOverSlot?: number | null;
  onTogglePlay: () => void;
  onStop: () => void;
  onToggleRecord: () => void;
  onSetBpm: (bpm: number) => void;
  onAdjustBpm: (delta: number) => void;
  onTapTempo: () => void;
  onToggleStep: (padIdx: number, stepIdx: number) => void;
  onClear: () => void;
  onChangeResolution: (res: StepResolution) => void;
  onTriggerPad?: (padIdx: number) => void;
  onToggleClick?: () => void;
  onToggleBank?: () => void;
  onToggleSwing?: () => void;
  onClearSelectedTrack?: (padIdx?: number) => void;
  onClearTrack?: (padIdx: number) => void;
  onSelectPad?: (padIdx: number) => void;
  isVisualMetronomeVisible?: boolean;
  onToggleVisualMetronome?: () => void;
  isCollapsed?: boolean;
  onToggleCollapse?: () => void;
  isMidiLearnActive?: boolean;
  selectedLearnTarget?: MidiLearnTarget | null;
  onSelectLearnTarget?: (target: MidiLearnTarget) => void;
  midiLearnMapping?: MidiLearnMapping;
}

const DRUM_KIT_NAMES = [
  "Kick 1 (36)",
  "Snare (38)",
  "Closed HH (42)",
  "Open HH (46)",
  "Low Tom (45)",
  "Mid Tom (47)",
  "Crash (49)",
  "Ride (51)",
];

// Paleta de Silicona Táctil por Pad (armonizada 1:1 con los Faders de mezcla F1-F8)
const PAD_ROLE_THEMES = [
  // Pad 1 (Kick) - Ámbar Neón
  {
    border: "border-amber-500/35",
    bg: "bg-gradient-to-b from-[#1c1710] via-[#12100d] to-[#0a0907]",
    text: "text-amber-300",
    shadow: "shadow-[0_0_10px_rgba(245,158,11,0.2),inset_0_1px_1px_rgba(255,255,255,0.08)]",
    selected: "border-2 border-amber-400 text-amber-100 shadow-[0_0_16px_rgba(245,158,11,0.6),inset_0_1px_1px_rgba(255,255,255,0.25)] font-black",
    hit: "bg-gradient-to-b from-amber-400 via-amber-500 to-amber-600 text-black border-2 border-white shadow-[0_0_24px_rgba(245,158,11,1),inset_0_0_10px_rgba(255,255,255,0.8)] font-black scale-[0.98]",
    led: "#f59e0b",
  },
  // Pad 2 (Snare) - Esmeralda
  {
    border: "border-emerald-500/35",
    bg: "bg-gradient-to-b from-[#101c15] via-[#0d130f] to-[#070b08]",
    text: "text-emerald-300",
    shadow: "shadow-[0_0_10px_rgba(16,185,129,0.2),inset_0_1px_1px_rgba(255,255,255,0.08)]",
    selected: "border-2 border-emerald-400 text-emerald-100 shadow-[0_0_16px_rgba(16,185,129,0.6),inset_0_1px_1px_rgba(255,255,255,0.25)] font-black",
    hit: "bg-gradient-to-b from-emerald-400 via-emerald-500 to-emerald-600 text-black border-2 border-white shadow-[0_0_24px_rgba(16,185,129,1),inset_0_0_10px_rgba(255,255,255,0.8)] font-black scale-[0.98]",
    led: "#10b981",
  },
  // Pad 3 (Clap) - Teal
  {
    border: "border-teal-500/35",
    bg: "bg-gradient-to-b from-[#101c1a] via-[#0d1312] to-[#070b0a]",
    text: "text-teal-300",
    shadow: "shadow-[0_0_10px_rgba(20,184,166,0.2),inset_0_1px_1px_rgba(255,255,255,0.08)]",
    selected: "border-2 border-teal-400 text-teal-100 shadow-[0_0_16px_rgba(20,184,166,0.6),inset_0_1px_1px_rgba(255,255,255,0.25)] font-black",
    hit: "bg-gradient-to-b from-teal-400 via-teal-500 to-teal-600 text-black border-2 border-white shadow-[0_0_24px_rgba(20,184,166,1),inset_0_0_10px_rgba(255,255,255,0.8)] font-black scale-[0.98]",
    led: "#14b8a6",
  },
  // Pad 4 (Closed Hat) - Cyan
  {
    border: "border-cyan-500/35",
    bg: "bg-gradient-to-b from-[#0e1a1e] via-[#0c1215] to-[#060a0c]",
    text: "text-cyan-300",
    shadow: "shadow-[0_0_10px_rgba(6,182,212,0.2),inset_0_1px_1px_rgba(255,255,255,0.08)]",
    selected: "border-2 border-cyan-400 text-cyan-100 shadow-[0_0_16px_rgba(6,182,212,0.6),inset_0_1px_1px_rgba(255,255,255,0.25)] font-black",
    hit: "bg-gradient-to-b from-cyan-400 via-cyan-500 to-cyan-600 text-black border-2 border-white shadow-[0_0_24px_rgba(6,182,212,1),inset_0_0_10px_rgba(255,255,255,0.8)] font-black scale-[0.98]",
    led: "#06b6d4",
  },
  // Pad 5 (Open Hat) - Sky
  {
    border: "border-sky-500/35",
    bg: "bg-gradient-to-b from-[#0e1820] via-[#0c1117] to-[#06090d]",
    text: "text-sky-300",
    shadow: "shadow-[0_0_10px_rgba(14,165,233,0.2),inset_0_1px_1px_rgba(255,255,255,0.08)]",
    selected: "border-2 border-sky-400 text-sky-100 shadow-[0_0_16px_rgba(14,165,233,0.6),inset_0_1px_1px_rgba(255,255,255,0.25)] font-black",
    hit: "bg-gradient-to-b from-sky-400 via-sky-500 to-sky-600 text-black border-2 border-white shadow-[0_0_24px_rgba(14,165,233,1),inset_0_0_10px_rgba(255,255,255,0.8)] font-black scale-[0.98]",
    led: "#0ea5e9",
  },
  // Pad 6 (Low Tom) - Orange
  {
    border: "border-orange-500/35",
    bg: "bg-gradient-to-b from-[#1e150f] via-[#140e0a] to-[#0b0805]",
    text: "text-orange-300",
    shadow: "shadow-[0_0_10px_rgba(249,115,22,0.2),inset_0_1px_1px_rgba(255,255,255,0.08)]",
    selected: "border-2 border-orange-400 text-orange-100 shadow-[0_0_16px_rgba(249,115,22,0.6),inset_0_1px_1px_rgba(255,255,255,0.25)] font-black",
    hit: "bg-gradient-to-b from-orange-400 via-orange-500 to-orange-600 text-black border-2 border-white shadow-[0_0_24px_rgba(249,115,22,1),inset_0_0_10px_rgba(255,255,255,0.8)] font-black scale-[0.98]",
    led: "#f97316",
  },
  // Pad 7 (High Tom) - Rose
  {
    border: "border-rose-500/35",
    bg: "bg-gradient-to-b from-[#1e1015] via-[#140b0e] to-[#0b0607]",
    text: "text-rose-300",
    shadow: "shadow-[0_0_10px_rgba(244,63,94,0.2),inset_0_1px_1px_rgba(255,255,255,0.08)]",
    selected: "border-2 border-rose-400 text-rose-100 shadow-[0_0_16px_rgba(244,63,94,0.6),inset_0_1px_1px_rgba(255,255,255,0.25)] font-black",
    hit: "bg-gradient-to-b from-rose-400 via-rose-500 to-rose-600 text-black border-2 border-white shadow-[0_0_24px_rgba(244,63,94,1),inset_0_0_10px_rgba(255,255,255,0.8)] font-black scale-[0.98]",
    led: "#f43f5e",
  },
  // Pad 8 (Crash) - Purple
  {
    border: "border-purple-500/35",
    bg: "bg-gradient-to-b from-[#191020] via-[#110b16] to-[#09060c]",
    text: "text-purple-300",
    shadow: "shadow-[0_0_10px_rgba(168,85,247,0.2),inset_0_1px_1px_rgba(255,255,255,0.08)]",
    selected: "border-2 border-purple-400 text-purple-100 shadow-[0_0_16px_rgba(168,85,247,0.6),inset_0_1px_1px_rgba(255,255,255,0.25)] font-black",
    hit: "bg-gradient-to-b from-purple-400 via-purple-500 to-purple-600 text-black border-2 border-white shadow-[0_0_24px_rgba(168,85,247,1),inset_0_0_10px_rgba(255,255,255,0.8)] font-black scale-[0.98]",
    led: "#a855f7",
  },
];

const STEPS_ARRAY = Array.from({ length: 16 }, (_, i) => i);

export const StepSequencer: React.FC<StepSequencerProps> = ({
  viewMode = "keys",
  onChangeViewMode,
  sequencerState,
  padSlots,
  padEngineMode,
  onChangePadEngineMode,
  activePads = new Map(),
  onTriggerPadWithNote,
  onReleasePadWithNote,
  onPickPadSample,
  onClearPadSlot,
  onTogglePadMode,
  onDropFileOnPad,
  dragOverSlot = null,
  onTogglePlay,
  onStop,
  onToggleRecord,
  onSetBpm,
  onAdjustBpm,
  onTapTempo,
  onToggleStep,
  onClear,
  onChangeResolution,
  onTriggerPad,
  onToggleClick,
  onToggleBank,
  onToggleSwing,
  onClearSelectedTrack,
  onClearTrack,
  onSelectPad,
  isVisualMetronomeVisible = false,
  onToggleVisualMetronome,
  isMidiLearnActive = false,
  selectedLearnTarget = null,
  onSelectLearnTarget,
  midiLearnMapping,
}) => {
  const {
    isPlaying,
    isRecording,
    bpm,
    currentStep,
    resolution,
    pattern,
    currentBank = "a",
    isClickEnabled = false,
    isSwingEnabled = false,
    selectedPad = 0,
  } = sequencerState;

  const [isEditingBpm, setIsEditingBpm] = useState(false);
  const [bpmInput, setBpmInput] = useState(bpm.toString());
  const [contextMenu, setContextMenu] = useState<{
    padIdx: number;
    x: number;
    y: number;
  } | null>(null);
  const contextMenuRef = useRef<HTMLDivElement>(null);
  const [isSf2MenuOpen, setIsSf2MenuOpen] = useState(false);
  const [activeSf2Category, setActiveSf2Category] = useState<string | null>(null);
  const [drumNotes, setDrumNotes] = useState<number[]>([36, 38, 42, 46, 39, 45, 48, 49]);
  const [customDrumNames, setCustomDrumNames] = useState<Record<number, string>>({});
  const [localDragOverPad, setLocalDragOverPad] = useState<number | null>(null);

  // Cargar notas de batería activas desde el backend
  useEffect(() => {
    invoke<number[]>("get_drum_pad_notes")
      .then((notes) => {
        if (Array.isArray(notes) && notes.length === 8) {
          setDrumNotes(notes);
        }
      })
      .catch(console.error);
  }, []);

  // Cerrar menú contextual al hacer clic fuera o pulsar Escape
  useEffect(() => {
    if (!contextMenu) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (contextMenuRef.current && !contextMenuRef.current.contains(event.target as Node)) {
        setContextMenu(null);
        setIsSf2MenuOpen(false);
        setActiveSf2Category(null);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setContextMenu(null);
        setIsSf2MenuOpen(false);
        setActiveSf2Category(null);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [contextMenu]);

  // Menú selector de Kits
  const [isKitMenuOpen, setIsKitMenuOpen] = useState(false);
  const kitMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (kitMenuRef.current && !kitMenuRef.current.contains(event.target as Node)) {
        setIsKitMenuOpen(false);
      }
    };
    if (isKitMenuOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isKitMenuOpen]);

  const handleAssignSf2Drum = (padIdx: number, note: number, sampleName: string) => {
    invoke("set_drum_pad_note", { padIndex: padIdx, note }).catch(console.error);
    setDrumNotes((prev) => {
      const next = [...prev];
      next[padIdx] = note;
      return next;
    });
    setCustomDrumNames((prev) => ({
      ...prev,
      [padIdx]: sampleName,
    }));
    if (padEngineMode !== "drumKitSf2") {
      onChangePadEngineMode?.("drumKitSf2");
    }
    setContextMenu(null);
    setIsSf2MenuOpen(false);
    setActiveSf2Category(null);
  };

  // Mapeo MIDI de notas por pad
  const PAD_MIDI_NOTES = [36, 37, 38, 39, 40, 41, 42, 43];

  const getPadName = (padIdx: number): string => {
    if (padEngineMode === "drumKitSf2") {
      if (customDrumNames[padIdx]) {
        return customDrumNames[padIdx];
      }
      const note = drumNotes[padIdx];
      for (const cat of SF2_DRUM_CATEGORIES) {
        const found = cat.samples.find((s) => s.note === note);
        if (found) return found.name;
      }
      return DRUM_KIT_NAMES[padIdx] || `Drum ${padIdx + 1}`;
    }
    const slot = padSlots[padIdx];
    if (slot && slot.sampleName) {
      return slot.sampleName.replace(/\.[^/.]+$/, "");
    }
    return `Pad ${padIdx + 1}`;
  };

  const handleBpmSubmit = () => {
    setIsEditingBpm(false);
    const parsed = parseFloat(bpmInput);
    if (!isNaN(parsed) && parsed >= 40 && parsed <= 300) {
      onSetBpm(Math.round(parsed * 10) / 10);
    } else {
      setBpmInput(bpm.toString());
    }
  };

  const isSeqOpen = viewMode === "seq";

  return (
    <div className="w-full flex flex-col bg-[#050507] border-t border-white/[0.08] select-none relative z-40">
      {/* 1. Bahía de Secuenciación Rítmica TR-Style (Visible únicamente en modo SEQ) */}
      {isSeqOpen && (
        <div className="p-3 sm:p-3.5 flex flex-col gap-1.5 animate-in fade-in duration-150">
          {/* 8 Filas de Pistas: Drum Pads de Silicona Táctil + 16 Botones TR-Style */}
          {[0, 1, 2, 3, 4, 5, 6, 7].map((padIdx) => {
            const padNum = padIdx + 1;
            const padName = getPadName(padIdx);
            const isSelected = selectedPad === padIdx;
            const isHit = activePads.has(padNum);
            const vel = activePads.get(padNum) || 0;
            const isDragging = dragOverSlot === padIdx || localDragOverPad === padNum;
            const midiNote = midiLearnMapping ? midiLearnMapping.pads[padIdx] : PAD_MIDI_NOTES[padIdx];
            const theme = PAD_ROLE_THEMES[padIdx] || PAD_ROLE_THEMES[0];
            const isLearnSelected = isMidiLearnActive && selectedLearnTarget?.type === "pad" && selectedLearnTarget.index === padIdx;

            return (
              <div key={`seq-row-${padIdx}`} className="flex items-center gap-1.5 group">
                {/* Pad de Silicona Gruesa Táctil en Bajo Relieve */}
                <div
                  data-pad-slot={padIdx}
                  onMouseDown={(e) => {
                    if (e.button === 0) {
                      if (isMidiLearnActive) {
                        if (onSelectLearnTarget) {
                          onSelectLearnTarget({ type: "pad", index: padIdx });
                        }
                        return;
                      }
                      if (onSelectPad) onSelectPad(padIdx);
                      if (onTriggerPadWithNote) {
                        onTriggerPadWithNote(padNum, midiNote);
                      } else if (onTriggerPad) {
                        onTriggerPad(padIdx);
                      }
                    }
                  }}
                  onMouseUp={(e) => {
                    if (e.button === 0 && onReleasePadWithNote) {
                      onReleasePadWithNote(padNum, midiNote);
                    }
                  }}
                  onMouseLeave={() => {
                    if (isHit && onReleasePadWithNote) {
                      onReleasePadWithNote(padNum, midiNote);
                    }
                  }}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    const x = Math.min(e.clientX, window.innerWidth - 450);
                    const y = Math.min(e.clientY, window.innerHeight - 320);
                    setContextMenu({ padIdx, x: Math.max(10, x), y: Math.max(10, y) });
                    setIsSf2MenuOpen(false);
                    setActiveSf2Category(null);
                  }}
                  onDragOver={(e) => {
                    e.preventDefault();
                    setLocalDragOverPad(padNum);
                  }}
                  onDragLeave={() => setLocalDragOverPad(null)}
                  onDrop={(e) => {
                    e.preventDefault();
                    setLocalDragOverPad(null);
                    const files = e.dataTransfer.files;
                    if (files.length > 0) {
                      const file = files[0];
                      const path = (file as any).path;
                      if (path && onDropFileOnPad) {
                        onDropFileOnPad(padIdx, path);
                      } else if (onPickPadSample) {
                        onPickPadSample(padIdx);
                      }
                    }
                  }}
                  title={
                    isMidiLearnActive
                      ? `Pad ${padNum}: Clic para aprender MIDI Note (actual: ${getMidiNoteName(midiNote)} / ${midiNote})`
                      : `Pad ${padNum}: ${padName}. Clic para tocar/seleccionar | Clic derecho para opciones | Arrastra WAV aquí.`
                  }
                  className={`w-36 sm:w-40 mr-2 h-9 sm:h-9.5 rounded-lg flex items-center justify-center transition-all shrink-0 cursor-pointer select-none border relative overflow-hidden active:scale-[0.98] ${
                    isLearnSelected
                      ? "border-2 border-amber-400 bg-amber-950/40 shadow-[0_0_20px_rgba(245,158,11,0.85)] ring-2 ring-amber-400/60 animate-pulse text-amber-100"
                      : isMidiLearnActive
                      ? "border-2 border-amber-500/60 shadow-[0_0_12px_rgba(245,158,11,0.35)] hover:border-amber-400 hover:shadow-[0_0_16px_rgba(245,158,11,0.6)] bg-gradient-to-b from-[#18140e] to-[#0c0a07] text-amber-200"
                      : isHit
                      ? theme.hit
                      : isDragging
                      ? "bg-cyan-950/80 border-2 border-cyan-400 text-cyan-200 scale-[1.02] shadow-[0_0_20px_rgba(34,211,238,0.9)]"
                      : isSelected
                      ? theme.selected
                      : `${theme.border} ${theme.bg} ${theme.text} ${theme.shadow} hover:brightness-125`
                  }`}
                >
                  {/* Nombre del Sample centrado */}
                  <span
                    title={`Pad ${padNum}: ${padName}`}
                    className="font-mono text-[9px] sm:text-[9.5px] font-bold tracking-tight truncate max-w-full px-2.5 text-center select-none"
                  >
                    {padName}
                  </span>

                  {/* Indicador de velocidad al golpear */}
                  {isHit && (
                    <span className="absolute bottom-0.5 left-1.5 text-[8.5px] font-mono text-black font-extrabold tabular-nums">
                      v:{vel || 127}
                    </span>
                  )}
                </div>

                {/* 16 Botones de Paso TR-Style (Cuerpo Mecánico Biselado + Lente LED Interno) */}
                <div className="flex-1 flex items-center gap-1">
                  {STEPS_ARRAY.map((stepIdx) => {
                    const velocity = pattern[padIdx]?.[stepIdx] || 0;
                    const isActive = velocity > 0;
                    const isPlayhead = isPlaying && currentStep === stepIdx;
                    const isGroupDivider = stepIdx > 0 && stepIdx % 4 === 0;
                    const isBeatStart = stepIdx % 4 === 0;
                    const isEvenGroup = Math.floor(stepIdx / 4) % 2 === 0;

                    return (
                      <React.Fragment key={`pad-${padIdx}-step-${stepIdx}`}>
                        {isGroupDivider && <div className="w-1.5" />}
                        <button
                          type="button"
                          onClick={() => onToggleStep(padIdx, stepIdx)}
                          title={`Pad ${padNum}, Paso ${stepIdx + 1}: ${
                            isActive ? `Activo (Vel ${velocity})` : "Inactivo"
                          }`}
                          className={`flex-1 min-w-[22px] sm:min-w-[26px] h-9 sm:h-9.5 rounded-[4px] font-mono text-[9.5px] transition-all relative overflow-hidden active:scale-95 cursor-pointer ${
                            isActive
                              ? isPlayhead
                                ? "bg-emerald-300 text-black border border-white shadow-[0_0_20px_#ffffff] z-10 brightness-110"
                                : "bg-gradient-to-b from-[#3a2f1c] via-[#2a2113] to-[#18130a] border-x border-[#0e0c07] border-t border-amber-500/70 border-b-[3px] border-b-[#090704] shadow-[0_3px_6px_rgba(0,0,0,0.8),inset_0_0_12px_rgba(245,158,11,0.35)] hover:brightness-125"
                              : isPlayhead
                              ? "bg-[#282d38] border border-emerald-400/80 shadow-[0_0_12px_rgba(16,185,129,0.4)]"
                              : isEvenGroup
                              ? "bg-gradient-to-b from-[#2a2d36] via-[#1e2027] to-[#14151a] border-x border-[#0e0f13] border-t border-white/[0.12] border-b-[3px] border-b-[#0a0b0e] shadow-[0_3px_6px_rgba(0,0,0,0.65),inset_0_1px_0_rgba(255,255,255,0.06)] hover:brightness-125"
                              : "bg-gradient-to-b from-[#21232b] via-[#17181e] to-[#0f1014] border-x border-[#0b0c0f] border-t border-white/[0.08] border-b-[3px] border-b-[#07080a] shadow-[0_3px_6px_rgba(0,0,0,0.65),inset_0_1px_0_rgba(255,255,255,0.04)] hover:brightness-125"
                          }`}
                        >
                          {/* Lente Óptica LED Interna Centrada */}
                          <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
                            <span
                              className={`w-4 h-1.5 rounded-sm transition-all ${
                                isActive
                                  ? isPlayhead
                                    ? "bg-white shadow-[0_0_16px_#ffffff] scale-110"
                                    : "bg-amber-400 shadow-[0_0_10px_#f59e0b,0_0_20px_rgba(245,158,11,0.9)]"
                                  : isPlayhead
                                  ? "bg-emerald-400 shadow-[0_0_10px_#10b981]"
                                  : isBeatStart
                                  ? "bg-white/35"
                                  : "bg-white/10"
                              }`}
                            />
                          </div>

                          {/* Marca táctil superior de Downbeat */}
                          {isBeatStart && !isActive && (
                            <span className="absolute top-1 left-1/2 -translate-x-1/2 w-1.5 h-0.5 rounded-full bg-white/35 pointer-events-none" />
                          )}
                        </button>
                      </React.Fragment>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* 2. Barra de Docking Inferior Maestro (Siempre visible, conmutador de modos, kit, y controles de transporte) */}
      <div className="px-3 py-2 sm:px-4 bg-gradient-to-b from-[#101115] via-[#0b0c0f] to-[#060709] border-t border-white/[0.08] flex items-center justify-between gap-2.5 flex-nowrap relative z-50 overflow-visible">
        {/* Lado Izquierdo: [ KEYS ] / [ SEQ ] + Kit + Banco A/B + Resolución + Extras */}
        <div className="flex items-center gap-2 shrink-0">
          {/* Conmutador Físico de Modo Compacto KEYS / SEQ */}
          <div className="flex items-center bg-[#040406] border border-white/15 rounded-md p-0.5 shadow-[inset_0_1px_3px_rgba(0,0,0,0.8)]">
            <button
              type="button"
              onClick={() => onChangeViewMode?.("keys")}
              title="Mostrar Teclado de 49 Teclas"
              className={`px-2.5 py-1 rounded font-mono text-[9.5px] font-bold tracking-wider transition-all cursor-pointer ${
                !isSeqOpen
                  ? "bg-gradient-to-b from-cyan-400 to-cyan-600 text-black shadow-[0_0_12px_rgba(6,182,212,0.6)] font-black"
                  : "text-zinc-400 hover:text-white hover:bg-white/5"
              }`}
            >
              KEYS
            </button>
            <button
              type="button"
              onClick={() => onChangeViewMode?.("seq")}
              title="Abrir Secuenciador Rítmico de 16 Pasos"
              className={`px-2.5 py-1 rounded font-mono text-[9.5px] font-bold tracking-wider transition-all cursor-pointer ${
                isSeqOpen
                  ? "bg-gradient-to-b from-amber-400 to-amber-600 text-black shadow-[0_0_12px_rgba(245,158,11,0.6)] font-black"
                  : "text-zinc-400 hover:text-white hover:bg-white/5"
              }`}
            >
              SEQ
            </button>
          </div>

          <div className="h-4 w-[1px] bg-white/10 mx-0.5" />

          {/* Selector de Kit reubicado en la barra */}
          <div ref={kitMenuRef} className="relative z-50">
            <button
              type="button"
              onClick={() => setIsKitMenuOpen(!isKitMenuOpen)}
              className="h-7 px-2 bg-[#0e0f14] hover:bg-[#161820] border border-white/15 hover:border-emerald-500/60 rounded-md flex items-center gap-1.5 text-[9.5px] font-mono transition shadow-[inset_0_1px_0_rgba(255,255,255,0.08),0_2px_4px_rgba(0,0,0,0.6)] group select-none cursor-pointer"
              title="Cambiar Kit de Batería"
            >
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_#10b981]" />
              <span className="text-[9.5px] uppercase font-bold tracking-wider text-zinc-400">KIT:</span>
              <span className="font-bold text-zinc-100 group-hover:text-emerald-400 transition max-w-[90px] truncate">
                {padEngineMode === "drumKitSf2" ? "Yamaha RX7" : "Custom WAV"}
              </span>
              <svg
                className={`w-2.5 h-2.5 text-zinc-400 group-hover:text-white transition-transform duration-150 shrink-0 ${
                  isKitMenuOpen ? "rotate-180" : ""
                }`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
              </svg>
            </button>

            {/* Menú Desplegable Flotante de Kits (hacia arriba, sobre el teclado) */}
            {isKitMenuOpen && (
              <div className="absolute bottom-full mb-2 left-0 z-50 w-52 bg-[#0d0e12] border border-white/20 rounded-xl shadow-[0_15px_40px_rgba(0,0,0,0.95),0_0_1px_1px_rgba(255,255,255,0.08)] py-1 text-xs font-mono text-zinc-200 animate-in fade-in zoom-in-95 duration-100">
                <div className="px-3 py-1 text-[9.5px] uppercase font-bold tracking-wider text-zinc-400 border-b border-white/10 mb-1">
                  Kits de Batería
                </div>
                <button
                  type="button"
                  onClick={() => {
                    onChangePadEngineMode?.("drumKitSf2");
                    setIsKitMenuOpen(false);
                  }}
                  className={`w-full text-left px-3 py-1.5 flex items-center justify-between transition-colors cursor-pointer text-[10px] ${
                    padEngineMode === "drumKitSf2"
                      ? "bg-emerald-500/20 text-emerald-400 font-bold"
                      : "hover:bg-white/5 hover:text-white"
                  }`}
                >
                  <span>Yamaha RX7 (SF2)</span>
                  {padEngineMode === "drumKitSf2" && <Check size={12} className="text-emerald-400" />}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    onChangePadEngineMode?.("wavSampler");
                    setIsKitMenuOpen(false);
                  }}
                  className={`w-full text-left px-3 py-1.5 flex items-center justify-between transition-colors cursor-pointer text-[10px] ${
                    padEngineMode === "wavSampler"
                      ? "bg-emerald-500/20 text-emerald-400 font-bold"
                      : "hover:bg-white/5 hover:text-white"
                  }`}
                >
                  <span>Custom WAV (Pads)</span>
                  {padEngineMode === "wavSampler" && <Check size={12} className="text-emerald-400" />}
                </button>
              </div>
            )}
          </div>

          {/* Selector de Banco Compacto: A / B */}
          <button
            type="button"
            onClick={onToggleBank}
            title={`Cambiar Banco (Actual: ${currentBank.toUpperCase()})`}
            className={`h-7 px-2.5 rounded-md text-[9.5px] font-mono font-bold tracking-wider border transition-all active:scale-95 shadow-[inset_0_1px_0_rgba(255,255,255,0.1),0_2px_4px_rgba(0,0,0,0.5)] cursor-pointer flex items-center gap-1.5 ${
              currentBank === "b"
                ? "bg-purple-900/40 text-purple-200 border-purple-500/60 shadow-[0_0_8px_rgba(168,85,247,0.4)]"
                : "bg-cyan-950/40 text-cyan-200 border-cyan-500/60 shadow-[0_0_8px_rgba(6,182,212,0.4)]"
            }`}
          >
            <span className="text-zinc-400 font-semibold text-[9px] tracking-wider">BANK</span>
            <span>{currentBank.toUpperCase()}</span>
          </button>

          {/* Selector de Resolución Compacto con Dropdown / Toggle Cíclico */}
          <div className="relative">
            <button
              type="button"
              onClick={() => {
                const nextRes: StepResolution =
                  resolution === "eighth"
                    ? "sixteenth"
                    : resolution === "sixteenth"
                    ? "thirtySecond"
                    : "eighth";
                onChangeResolution(nextRes);
              }}
              title="Resolución de tiempo (Clic para alternar 1/8, 1/16, 1/32)"
              className="h-7 px-2.5 bg-[#040406] hover:bg-[#121318] border border-white/15 rounded-md text-[9.5px] font-mono font-bold text-emerald-400 flex items-center gap-1.5 shadow-[inset_0_1px_2px_rgba(0,0,0,0.8)] cursor-pointer transition"
            >
              <span className="text-zinc-400 text-[9px] font-semibold tracking-wider">GRID</span>
              <span>{resolution === "eighth" ? "1/8" : resolution === "sixteenth" ? "1/16" : "1/32"}</span>
              <ChevronDown size={11} className="text-zinc-400" />
            </button>
          </div>

          {/* Swing MPC 58% */}
          {onToggleSwing && (
            <button
              type="button"
              onClick={onToggleSwing}
              title="Groove Swing MPC 58% ON/OFF"
              className={`h-7 px-2 rounded-md text-[9.5px] font-mono font-bold tracking-wider border transition-all flex items-center gap-1 active:scale-95 cursor-pointer ${
                isSwingEnabled
                  ? "bg-pink-950/40 text-pink-300 border-pink-500/60 shadow-[0_0_8px_rgba(236,72,153,0.4)]"
                  : "bg-white/[0.03] text-zinc-400 border-white/10 hover:text-white"
              }`}
            >
              <span className={`w-1.5 h-1.5 rounded-full ${isSwingEnabled ? "bg-pink-400 shadow-[0_0_4px_#ec4899]" : "bg-zinc-600"}`} />
              SWING
            </button>
          )}

          {/* Clic Metrónomo */}
          {onToggleClick && (
            <button
              type="button"
              onClick={onToggleClick}
              title="Clic Metrónomo ON/OFF"
              className={`h-7 px-2 rounded-md text-[9.5px] font-mono font-bold tracking-wider border transition-all flex items-center gap-1 active:scale-95 cursor-pointer ${
                isClickEnabled
                  ? "bg-amber-950/40 text-amber-300 border-amber-500/60 shadow-[0_0_8px_rgba(245,158,11,0.4)]"
                  : "bg-white/[0.03] text-zinc-400 border-white/10 hover:text-white"
              }`}
            >
              <span className={`w-1.5 h-1.5 rounded-full ${isClickEnabled ? "bg-amber-400 shadow-[0_0_4px_#f59e0b]" : "bg-zinc-600"}`} />
              CLIC
            </button>
          )}
        </div>

        {/* Lado Derecho: Display VFD de BPM y Transporte Maestro */}
        <div className="flex items-center gap-2.5">
          {/* Display VFD de Tempo (Pantalla Digital Hardware) */}
          <div className="flex items-center bg-[#030305] border border-white/15 rounded-lg p-0.5 shadow-[inset_0_2px_6px_rgba(0,0,0,0.9),0_1px_0_rgba(255,255,255,0.05)]">
            <button
              type="button"
              onClick={() => onAdjustBpm(-1)}
              title="Disminuir Tempo"
              className="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/10 rounded font-mono font-bold text-xs transition cursor-pointer"
            >
              -
            </button>

            <div className="flex items-baseline px-2.5 border-x border-white/10 mx-0.5">
              {isEditingBpm ? (
                <input
                  type="number"
                  min="40"
                  max="300"
                  step="1"
                  autoFocus
                  value={bpmInput}
                  onChange={(e) => setBpmInput(e.target.value)}
                  onBlur={handleBpmSubmit}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleBpmSubmit();
                    if (e.key === "Escape") {
                      setIsEditingBpm(false);
                      setBpmInput(bpm.toString());
                    }
                  }}
                  className="w-12 text-center bg-black border border-emerald-400 rounded text-emerald-300 font-mono text-xs font-bold outline-none px-0.5 tabular-nums"
                />
              ) : (
                <span
                  onClick={() => {
                    setBpmInput(Math.round(bpm).toString());
                    setIsEditingBpm(true);
                  }}
                  title="Clic para editar tempo"
                  className="font-mono text-xs font-bold text-emerald-400 tracking-wider cursor-pointer hover:brightness-125 transition select-none drop-shadow-[0_0_6px_rgba(16,185,129,0.5)] tabular-nums"
                >
                  {Math.round(bpm)}
                </span>
              )}
              <span className="text-[9px] font-mono font-bold text-zinc-400 uppercase tracking-wider ml-1">BPM</span>
            </div>

            <button
              type="button"
              onClick={() => onAdjustBpm(1)}
              title="Aumentar Tempo"
              className="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/10 rounded font-mono font-bold text-xs transition cursor-pointer"
            >
              +
            </button>

            <button
              type="button"
              onClick={onTapTempo}
              title="Tap Tempo"
              className="px-2 py-1 ml-1 text-[9.5px] font-mono font-bold uppercase tracking-wider rounded bg-white/[0.06] hover:bg-emerald-500 hover:text-black text-zinc-300 border border-white/10 transition active:scale-95 cursor-pointer"
            >
              TAP
            </button>
          </div>

          <div className="h-4 w-[1px] bg-white/10 mx-0.5 hidden sm:block" />

          {/* Bloque de Transporte Maestro (STOP, PLAY, REC) */}
          <div className="flex items-center gap-1.5">
            {/* Stop */}
            <button
              type="button"
              onClick={onStop}
              title="Stop / Reset a paso 1"
              className="p-2 rounded-lg bg-[#18191e] hover:bg-[#24262d] text-zinc-400 hover:text-white border border-white/15 transition active:scale-95 shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_2px_4px_rgba(0,0,0,0.5)] cursor-pointer"
            >
              <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                <path d="M6 6h12v12H6z" />
              </svg>
            </button>

            {/* Play / Pause */}
            <button
              type="button"
              onClick={onTogglePlay}
              title="Play / Pause"
              className={`px-4 py-1.5 rounded-lg text-[11px] font-mono font-bold uppercase tracking-wider flex items-center gap-1.5 transition active:scale-95 border cursor-pointer ${
                isPlaying
                  ? "bg-gradient-to-b from-emerald-400 to-emerald-600 text-black border-emerald-300 shadow-[0_0_16px_rgba(16,185,129,0.7),inset_0_1px_0_rgba(255,255,255,0.6)]"
                  : "bg-[#18191e] hover:bg-[#24262d] text-zinc-200 hover:text-white border-white/15 shadow-[inset_0_1px_0_rgba(255,255,255,0.08),0_2px_6px_rgba(0,0,0,0.6)]"
              }`}
            >
              {isPlaying ? (
                <>
                  <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                    <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
                  </svg>
                  PAUSE
                </>
              ) : (
                <>
                  <svg className="w-3.5 h-3.5 text-emerald-400" fill="currentColor" viewBox="0 0 24 24">
                    <path d="M8 5v14l11-7z" />
                  </svg>
                  PLAY
                </>
              )}
            </button>

            {/* Overdub Record */}
            <button
              type="button"
              onClick={onToggleRecord}
              title="Overdub Record"
              className={`px-3 py-1.5 rounded-lg text-[11px] font-mono font-bold uppercase tracking-wider flex items-center gap-1.5 transition border active:scale-95 cursor-pointer shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_2px_4px_rgba(0,0,0,0.5)] ${
                isRecording
                  ? "bg-red-500/25 border-red-500 text-red-300 shadow-[0_0_14px_rgba(239,68,68,0.6)] animate-pulse"
                  : "bg-[#18191e] border-white/15 text-zinc-400 hover:text-zinc-200 hover:bg-[#24262d]"
              }`}
            >
              <span
                className={`w-2 h-2 rounded-full ${
                  isRecording ? "bg-red-500 shadow-[0_0_6px_#ef4444]" : "bg-zinc-500"
                }`}
              />
              REC
            </button>
          </div>

          <div className="h-4 w-[1px] bg-white/10 mx-0.5 hidden sm:block" />

          {/* Overlay Metrónomo */}
          {onToggleVisualMetronome && (
            <button
              type="button"
              onClick={onToggleVisualMetronome}
              title="Metrónomo Visual Overlay ON/OFF"
              className={`px-2.5 py-1.5 rounded-md text-[9.5px] font-mono font-bold tracking-wider border transition flex items-center gap-1.5 active:scale-95 cursor-pointer ${
                isVisualMetronomeVisible
                  ? "bg-emerald-950/40 text-emerald-300 border-emerald-500/50 shadow-[0_0_6px_rgba(16,185,129,0.3)]"
                  : "bg-white/[0.03] text-zinc-400 border-white/10 hover:text-white"
              }`}
            >
              <span className={`w-1.5 h-1.5 rounded-full ${isVisualMetronomeVisible ? "bg-emerald-400 shadow-[0_0_4px_#10b981]" : "bg-zinc-600"}`} />
              OVERLAY
            </button>
          )}

          {/* Limpiar Patrón Activo */}
          <button
            type="button"
            onClick={onClear}
            title="Limpiar patrón activo"
            className="p-2 rounded-lg bg-[#18191e] text-zinc-400 hover:text-red-400 hover:bg-[#24262d] border border-white/10 transition active:scale-95 cursor-pointer shadow-[inset_0_1px_0_rgba(255,255,255,0.05),0_1px_3px_rgba(0,0,0,0.5)]"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
              />
            </svg>
          </button>
        </div>
      </div>

      {/* Menú Contextual Noir para Pads (Clic Derecho) */}
      {contextMenu && (
        <div
          ref={contextMenuRef}
          className="fixed z-50 flex items-start animate-in fade-in zoom-in-95 duration-100 select-none"
          style={{ top: contextMenu.y, left: contextMenu.x }}
          onClick={(e) => e.stopPropagation()}
        >
          {/* Menú Principal */}
          <div className="bg-[#0e0e12]/95 backdrop-blur-2xl border border-white/10 rounded-xl shadow-[0_20px_50px_rgba(0,0,0,0.95),inset_0_1px_0_rgba(255,255,255,0.08)] py-1 w-52 text-xs font-mono text-zinc-200">
            <div className="px-3 py-1.5 text-[10px] uppercase font-bold tracking-wider text-zinc-400 border-b border-white/5 mb-1 flex items-center justify-between">
              <span>Pad {contextMenu.padIdx + 1}</span>
              <span className="text-[9.5px] text-emerald-400 font-bold">Config</span>
            </div>

            <button
              type="button"
              className="w-full text-left px-3 py-1.5 hover:bg-white/10 hover:text-white flex items-center justify-between transition-colors cursor-pointer text-[11px]"
              onClick={() => {
                const idx = contextMenu.padIdx;
                setContextMenu(null);
                setIsSf2MenuOpen(false);
                setActiveSf2Category(null);
                onPickPadSample?.(idx);
              }}
            >
              <span>Cargar sample WAV...</span>
              <FolderOpen size={13} className="text-zinc-400 shrink-0" />
            </button>

            {/* Asignar instrumento SF2 con submenú en cascada */}
            <button
              type="button"
              className={`w-full text-left px-3 py-1.5 flex items-center justify-between transition-colors cursor-pointer text-[11px] ${
                isSf2MenuOpen ? "bg-white/10 text-white" : "hover:bg-white/10 hover:text-white"
              }`}
              onMouseEnter={() => setIsSf2MenuOpen(true)}
              onClick={() => setIsSf2MenuOpen((prev) => !prev)}
            >
              <div className="flex items-center gap-1.5">
                <Music size={12} className="text-emerald-400 shrink-0" />
                <span>Asignar sonido SF2</span>
              </div>
              <ChevronRight size={12} className="text-zinc-400 shrink-0" />
            </button>

            {padEngineMode === "wavSampler" && (
              <button
                type="button"
                className="w-full text-left px-3 py-1.5 hover:bg-white/10 hover:text-white flex items-center justify-between transition-colors cursor-pointer text-[11px]"
                onClick={() => {
                  const idx = contextMenu.padIdx;
                  setContextMenu(null);
                  setIsSf2MenuOpen(false);
                  setActiveSf2Category(null);
                  onTogglePadMode?.(idx);
                }}
              >
                <span>
                  Modo: {padSlots?.[contextMenu.padIdx]?.mode === "gate" ? "One-Shot" : "Gate"}
                </span>
                <RotateCcw size={13} className="text-zinc-400 shrink-0" />
              </button>
            )}

            {/* Opción para limpiar la pista de notas de este Pad en el patrón */}
            <button
              type="button"
              className="w-full text-left px-3 py-1.5 hover:bg-amber-950/50 hover:text-amber-300 text-amber-400 flex items-center justify-between transition-colors border-t border-white/5 cursor-pointer text-[11px]"
              onClick={() => {
                const idx = contextMenu.padIdx;
                setContextMenu(null);
                setIsSf2MenuOpen(false);
                setActiveSf2Category(null);
                if (onClearTrack) {
                  onClearTrack(idx);
                } else if (onClearSelectedTrack) {
                  onSelectPad?.(idx);
                  onClearSelectedTrack(idx);
                }
              }}
            >
              <span>Limpiar notas de pista</span>
              <Eraser size={13} className="text-amber-400/90 shrink-0" />
            </button>

            <button
              type="button"
              className="w-full text-left px-3 py-1.5 hover:bg-red-950/60 hover:text-red-300 text-red-400 flex items-center justify-between transition-colors border-t border-white/5 cursor-pointer text-[11px]"
              onClick={() => {
                const idx = contextMenu.padIdx;
                setContextMenu(null);
                setIsSf2MenuOpen(false);
                setActiveSf2Category(null);
                onClearPadSlot?.(idx);
              }}
            >
              <span>Vaciar slot</span>
              <Trash2 size={13} className="text-red-400/90 shrink-0" />
            </button>
          </div>

          {/* Submenú Nivel 1: Categorías (Sin emojis, nombres concisos en mayúsculas) */}
          {isSf2MenuOpen && (
            <div className="ml-1 bg-[#0e0e12]/95 backdrop-blur-2xl border border-white/10 rounded-xl shadow-[0_20px_50px_rgba(0,0,0,0.95),inset_0_1px_0_rgba(255,255,255,0.08)] py-1 w-36 text-xs font-mono text-zinc-200 animate-in fade-in duration-100">
              <div className="px-2.5 py-1 text-[9.5px] uppercase font-bold tracking-wider text-zinc-400 border-b border-white/5 mb-1 font-sans">
                CATEGORÍAS
              </div>
              {SF2_DRUM_CATEGORIES.map((cat) => {
                const isActive = activeSf2Category === cat.id;
                return (
                  <button
                    key={cat.id}
                    type="button"
                    className={`w-full text-left px-2.5 py-1.5 flex items-center justify-between transition-colors cursor-pointer text-[10px] font-sans font-medium uppercase tracking-wider ${
                      isActive
                        ? "bg-emerald-500/20 text-emerald-300 font-bold"
                        : "text-zinc-300 hover:bg-white/[0.06] hover:text-white"
                    }`}
                    onMouseEnter={() => setActiveSf2Category(cat.id)}
                    onClick={() => setActiveSf2Category(cat.id)}
                  >
                    <span>{cat.name}</span>
                    <ChevronRight size={11} className={isActive ? "text-emerald-400" : "text-zinc-500"} />
                  </button>
                );
              })}
            </div>
          )}

          {/* Submenú Nivel 2: Lista de Sonidos de la Categoría Seleccionada */}
          {isSf2MenuOpen && activeSf2Category && (
            <div className="ml-1 bg-[#0e0e12]/95 backdrop-blur-2xl border border-white/10 rounded-xl shadow-[0_20px_50px_rgba(0,0,0,0.95),inset_0_1px_0_rgba(255,255,255,0.08)] py-1 w-48 text-xs font-mono text-zinc-200 animate-in fade-in duration-100 max-h-72 overflow-y-auto">
              <div className="px-2.5 py-1 text-[9.5px] uppercase font-bold tracking-wider text-emerald-400 border-b border-white/5 mb-1 font-sans">
                {SF2_DRUM_CATEGORIES.find((c) => c.id === activeSf2Category)?.name}
              </div>
              {SF2_DRUM_CATEGORIES.find((c) => c.id === activeSf2Category)?.samples.map((sample) => {
                const isCurrentNote = drumNotes[contextMenu.padIdx] === sample.note;
                return (
                  <button
                    key={sample.note}
                    type="button"
                    className={`w-full text-left px-2.5 py-1.5 flex items-center justify-between transition-colors cursor-pointer text-[10.5px] font-sans ${
                      isCurrentNote
                        ? "bg-emerald-500/15 text-emerald-300 font-semibold"
                        : "text-zinc-300 hover:bg-white/[0.08] hover:text-white"
                    }`}
                    onClick={() => handleAssignSf2Drum(contextMenu.padIdx, sample.note, sample.name)}
                  >
                    <span className="truncate pr-1">{sample.name}</span>
                    <div className="flex items-center gap-1 shrink-0">
                      <span className="text-[9px] font-mono text-zinc-500 tabular-nums">[{sample.note}]</span>
                      {isCurrentNote && <Check size={11} className="text-emerald-400" />}
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
};
