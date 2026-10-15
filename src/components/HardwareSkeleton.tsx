import React, { useState, useEffect } from "react";
import { Check } from "lucide-react";
import { PadSlotInfo, KnobMappingInfo, KnobTarget, FxRouting } from "../types/sampler";
import { MidiLearnTarget, MidiLearnMapping } from "../types/midiLearn";

import { SoundFontInfo } from "../types/soundfont";
import { QWERTY_BASE_NOTE, QWERTY_NOTE_OFFSETS } from "../hooks/useQwertyKeyboard";
import { PerformanceBay } from "./PerformanceBay";

interface HardwareSkeletonProps {
  activeNotes: Map<number, number>;
  faders: number[]; // 9 values, 0-127
  knobs: number[]; // 8 values, 0-127
  pitchBend: number; // -8192 to 8191
  modWheel: number; // 0 to 127
  keyRange?: { minNote: number; maxNote: number; label?: string };
  padSlots?: PadSlotInfo[];
  knobMappings?: KnobMappingInfo[];
  onSetKnobMapping?: (knobIndex: number, target: KnobTarget) => void;
  fxRouting?: FxRouting;
  onChangeFxRouting?: (routing: FxRouting) => void;
  availableSoundfonts?: SoundFontInfo[];
  currentSoundfont?: SoundFontInfo | null;
  activePresetIndex?: number;
  onSelectSoundfont?: (filename: string) => void;
  onSelectPreset?: (patch: number, index: number, bank?: number) => void;
  onTriggerNote: (note: number, velocity: number) => void;
  onReleaseNote: (note: number) => void;
  onChangeFader: (index: number, value: number) => void;
  onChangeKnob: (index: number, value: number) => void;
  onChangePitchBend: (normalized: number) => void;
  onChangeModWheel: (value: number) => void;
  hideKeyboard?: boolean;
  isMidiLearnActive?: boolean;
  selectedLearnTarget?: MidiLearnTarget | null;
  onSelectLearnTarget?: (target: MidiLearnTarget) => void;
  midiLearnMapping?: MidiLearnMapping;
  qwertyEnabled?: boolean;
  qwertyOctaveOffset?: number;
  children?: React.ReactNode;
}

interface KeyItem {
  note: number;
  isBlack: boolean;
  isActive: boolean;
  velocity: number;
  isOutOfRange: boolean;
}

const FADER_PAD_COLORS = [
  { line: "bg-amber-400 shadow-[0_0_6px_#f59e0b]", dot: "bg-amber-400", border: "border-amber-400/40" }, // P1
  { line: "bg-emerald-400 shadow-[0_0_6px_#10b981]", dot: "bg-emerald-400", border: "border-emerald-400/40" }, // P2
  { line: "bg-teal-400 shadow-[0_0_6px_#14b8a6]", dot: "bg-teal-400", border: "border-teal-400/40" }, // P3
  { line: "bg-cyan-400 shadow-[0_0_6px_#06b6d4]", dot: "bg-cyan-400", border: "border-cyan-400/40" }, // P4
  { line: "bg-sky-400 shadow-[0_0_6px_#0ea5e9]", dot: "bg-sky-400", border: "border-sky-400/40" }, // P5
  { line: "bg-orange-400 shadow-[0_0_6px_#f97316]", dot: "bg-orange-400", border: "border-orange-400/40" }, // P6
  { line: "bg-rose-400 shadow-[0_0_6px_#f43f5e]", dot: "bg-rose-400", border: "border-rose-400/40" }, // P7
  { line: "bg-purple-400 shadow-[0_0_6px_#a855f7]", dot: "bg-purple-400", border: "border-purple-400/40" }, // P8
  { line: "bg-cyan-400 shadow-[0_0_8px_#22d3ee]", dot: "bg-cyan-400", border: "border-cyan-400/50" }, // Master F9
];

export const HardwareSkeleton: React.FC<HardwareSkeletonProps> = ({
  activeNotes,
  faders,
  knobs,
  pitchBend,
  modWheel,
  keyRange,
  padSlots = [],
  knobMappings = [],
  onSetKnobMapping,
  fxRouting = "both",
  onChangeFxRouting,
  availableSoundfonts = [],
  currentSoundfont,
  activePresetIndex = 0,
  onSelectSoundfont,
  onSelectPreset,
  onTriggerNote,
  onReleaseNote,
  onChangeFader,
  onChangeKnob,
  onChangePitchBend,
  onChangeModWheel,
  hideKeyboard = false,
  isMidiLearnActive = false,
  selectedLearnTarget = null,
  onSelectLearnTarget,
  midiLearnMapping,
  qwertyEnabled = false,
  qwertyOctaveOffset = 0,
  children,
}) => {
  // Mapa de notas a letra física QWERTY según la octava actual
  const qwertyLabels = React.useMemo(() => {
    if (!qwertyEnabled) return new Map<number, string>();

    const map = new Map<number, string>();
    const base = QWERTY_BASE_NOTE + qwertyOctaveOffset * 12;

    const codeToLabel: Record<string, string> = {
      KeyA: "A",
      KeyW: "W",
      KeyS: "S",
      KeyE: "E",
      KeyD: "D",
      KeyF: "F",
      KeyT: "T",
      KeyG: "G",
      KeyY: "Y",
      KeyH: "H",
      KeyU: "U",
      KeyJ: "J",
      KeyK: "K",
      KeyO: "O",
      KeyL: "L",
      KeyP: "P",
      Semicolon: ";",
    };

    for (const [code, offset] of Object.entries(QWERTY_NOTE_OFFSETS)) {
      const note = base + offset;
      if (note >= 36 && note <= 84) {
        if (!keyRange || (note >= keyRange.minNote && note <= keyRange.maxNote)) {
          map.set(note, codeToLabel[code] || code);
        }
      }
    }

    return map;
  }, [qwertyEnabled, qwertyOctaveOffset, keyRange]);

  const [knobContextMenu, setKnobContextMenu] = useState<{
    knobIdx: number;
    x: number;
    y: number;
  } | null>(null);

  useEffect(() => {
    const handleClickOutside = () => {
      setKnobContextMenu(null);
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setKnobContextMenu(null);
      }
    };
    window.addEventListener("click", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("click", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  const AVAILABLE_KNOB_TARGETS: { target: KnobTarget; label: string; desc: string }[] = [
    { target: "cutoff", label: "CUTOFF", desc: "Filtro Paso Bajo (40Hz - 18kHz)" },
    { target: "resonance", label: "RES", desc: "Filtro Resonancia Q" },
    { target: "drive", label: "DRIVE", desc: "Warm Saturation Analógica" },
    { target: "reverbMix", label: "REVERB", desc: "Reverb Freeverb Estéreo" },
    { target: "delayTime", label: "D-TIME", desc: "Delay Tiempo (40ms - 1s)" },
    { target: "delayFeedback", label: "D-FDBK", desc: "Delay Feedback / Ecos" },
    { target: "delayMix", label: "D-MIX", desc: "Delay Mix (Dry / Wet)" },
    { target: "chorusMix", label: "CHORUS", desc: "Chorus Modulación Estéreo" },
    { target: "pan", label: "PAN", desc: "Panorámica L/R Máster" },
  ];

  const DEFAULT_KNOB_TARGETS: KnobTarget[] = [
    "cutoff",
    "resonance",
    "drive",
    "reverbMix",
    "delayTime",
    "delayFeedback",
    "delayMix",
    "pan",
  ];

  const getKnobDefaultValue = (target: KnobTarget): number => {
    switch (target) {
      case "cutoff":
      case "resonance":
      case "pan":
        return 64;
      case "drive":
      case "reverbMix":
      case "delayTime":
      case "delayFeedback":
      case "delayMix":
      case "chorusMix":
        return 0;
      default:
        return 64;
    }
  };

  // Generar las 49 teclas (C1 / nota 36 a C5 / nota 84)
  const keys: KeyItem[] = [];
  const startNote = 36; // C1
  const numKeys = 49;

  for (let i = 0; i < numKeys; i++) {
    const note = startNote + i;
    const noteInOctave = note % 12;
    const isBlack = [1, 3, 6, 8, 10].includes(noteInOctave);
    const isActive = activeNotes.has(note);
    const velocity = activeNotes.get(note) || 0;
    const isOutOfRange =
      keyRange !== undefined &&
      (note < keyRange.minNote || note > keyRange.maxNote);

    keys.push({
      note,
      isBlack,
      isActive,
      velocity,
      isOutOfRange,
    });
  }





  // Drag handlers para Faders
  const handleFaderPointerDown = (index: number, e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);

    const updateFromPointer = (ev: PointerEvent | React.PointerEvent) => {
      const rect = el.getBoundingClientRect();
      const relativeY = ev.clientY - rect.top;
      const pct = 1.0 - Math.min(Math.max(relativeY / rect.height, 0), 1);
      const val = Math.round(pct * 127);
      onChangeFader(index, val);
    };

    updateFromPointer(e);

    const onPointerMove = (ev: PointerEvent) => updateFromPointer(ev);
    const onPointerUp = (ev: PointerEvent) => {
      el.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  // Drag handlers para Knobs (arrastre vertical estilo DAW)
  const handleKnobPointerDown = (index: number, e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const startY = e.clientY;
    const startVal = knobs[index];
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);

    const onPointerMove = (ev: PointerEvent) => {
      const deltaY = startY - ev.clientY; // Arriba suma, abajo resta
      const deltaVal = Math.round((deltaY / 100) * 127);
      const newVal = Math.min(Math.max(startVal + deltaVal, 0), 127);
      onChangeKnob(index, newVal);
    };

    const onPointerUp = (ev: PointerEvent) => {
      el.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  // Drag handlers para Pitch Wheel (con resorte de retorno automático al centro 0)
  const handlePitchWheelPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);

    const updatePitch = (ev: PointerEvent | React.PointerEvent) => {
      const rect = el.getBoundingClientRect();
      const relativeY = ev.clientY - rect.top;
      const normalizedRatio = 1.0 - 2.0 * Math.min(Math.max(relativeY / rect.height, 0), 1);
      const bend = Math.round(normalizedRatio * 8191);
      onChangePitchBend(bend);
    };

    updatePitch(e);

    const onPointerMove = (ev: PointerEvent) => updatePitch(ev);
    const onPointerUp = (ev: PointerEvent) => {
      el.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      // Retorno con resorte a 0
      onChangePitchBend(0);
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  // Drag handlers para Mod Wheel
  const handleModWheelPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);

    const updateMod = (ev: PointerEvent | React.PointerEvent) => {
      const rect = el.getBoundingClientRect();
      const relativeY = ev.clientY - rect.top;
      const pct = 1.0 - Math.min(Math.max(relativeY / rect.height, 0), 1);
      onChangeModWheel(Math.round(pct * 127));
    };

    updateMod(e);

    const onPointerMove = (ev: PointerEvent) => updateMod(ev);
    const onPointerUp = (ev: PointerEvent) => {
      el.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  return (
    <div className="bg-[#050508] border border-white/[0.12] rounded-2xl shadow-[0_20px_60px_rgba(0,0,0,0.95),0_0_1px_1px_rgba(255,255,255,0.06),inset_0_1px_0_0_rgba(255,255,255,0.1)] flex flex-col overflow-hidden select-none transition-all duration-200">
      {/* 1. Deck Superior del Hardware: Panel de Control Unificado (Wheels, Faders Matrix y Knobs Rack) */}
      <div className="p-3.5 md:p-4 bg-gradient-to-b from-[#111216] via-[#0d0e12] to-[#08080a] border-b border-white/[0.08] flex flex-col lg:flex-row gap-3 lg:gap-4 items-stretch lg:items-center w-full">
        {/* Sección Ruedas (Pitch Bend & Mod Wheel) */}
        <div className="flex items-center gap-2 pr-3 lg:pr-4 pb-3 lg:pb-0 border-b lg:border-b-0 lg:border-r border-white/[0.08] shrink-0">
          {/* Pitch Wheel */}
          <div className="flex flex-col items-center">
            <span className="text-[9.5px] font-mono text-zinc-400 mb-1 font-bold tracking-wider">PITCH</span>
            <div
              onPointerDown={handlePitchWheelPointerDown}
              className="w-6 h-28 bg-[#08080a] rounded-lg border border-white/[0.08] relative overflow-hidden flex items-center justify-center cursor-ns-resize shadow-[inset_0_2px_6px_rgba(0,0,0,0.9)] hover:border-white/20 transition"
              title="Pitch Bend"
            >
              {/* Estrías de goma física en la rueda */}
              <div className="absolute inset-0 flex flex-col justify-between py-2 pointer-events-none opacity-20">
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
              </div>
              {/* Centro neutro */}
              <div className="absolute w-full h-[1px] bg-white/30 top-1/2 shadow-[0_0_4px_white]" />
              {/* Indicador */}
              <div
                className="absolute w-full h-3 bg-cyan-400/90 rounded transition-all duration-75 shadow-[0_0_10px_rgba(34,211,238,0.8)]"
                style={{
                  top: `${50 - (pitchBend / 8192) * 40}%`,
                }}
              />
            </div>
            <span className="text-[9.5px] font-mono text-zinc-400 mt-1 font-medium tabular-nums">
              {pitchBend >= 0 ? `+${pitchBend}` : pitchBend}
            </span>
          </div>

          {/* Modulation Wheel */}
          <div className="flex flex-col items-center">
            <span className="text-[9.5px] font-mono text-zinc-400 mb-1 font-bold tracking-wider">MOD</span>
            <div
              onPointerDown={handleModWheelPointerDown}
              className="w-6 h-28 bg-[#08080a] rounded-lg border border-white/[0.08] relative overflow-hidden flex items-end cursor-ns-resize shadow-[inset_0_2px_6px_rgba(0,0,0,0.9)] hover:border-white/20 transition"
              title="Modulation (CC 1)"
            >
              {/* Estrías de goma física en la rueda */}
              <div className="absolute inset-0 flex flex-col justify-between py-2 pointer-events-none opacity-20 z-0">
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
                <div className="w-full h-[1px] bg-white" />
              </div>
              <div
                className="w-full bg-emerald-500 rounded transition-all duration-75 shadow-[0_0_10px_rgba(16,185,129,0.8)] z-10"
                style={{
                  height: `${(modWheel / 127) * 100}%`,
                }}
              />
            </div>
            <span className="text-[9.5px] font-mono text-zinc-400 mt-1 font-medium tabular-nums">{modWheel}</span>
          </div>
        </div>

        {/* Sección Faders Matrix (F1 a F9) */}
        <div className="w-full lg:w-[340px] shrink-0 pr-0 lg:pr-4 pb-3 lg:pb-0 border-b lg:border-b-0 lg:border-r border-white/[0.08]">
          <div className="flex justify-between items-center mb-2 px-0.5">
            <span className="text-[9.5px] font-mono uppercase tracking-wider text-zinc-400 font-bold">
              Faders Matrix
            </span>
            <span className="text-[9px] font-mono text-cyan-400 font-bold tracking-wide">
              F9 Master
            </span>
          </div>
          <div className="flex justify-between items-center gap-1">
            {faders.map((val, idx) => {
              const isMaster = idx === 8;
              const heightPct = (val / 127) * 100;
              const padSample = idx < 8 ? padSlots[idx]?.sampleName : null;
              const colorTheme = FADER_PAD_COLORS[idx] || FADER_PAD_COLORS[0];
              const defaultFaderVal = isMaster ? 100 : 64;

              const isTargetSelected =
                isMidiLearnActive &&
                selectedLearnTarget?.type === "fader" &&
                selectedLearnTarget?.index === idx;
              const mappedCc = midiLearnMapping?.faders[idx] ?? (isMaster ? 46 : 38 + idx);

              const handleFaderReset = (e: React.MouseEvent) => {
                e.stopPropagation();
                if (!isMidiLearnActive) {
                  onChangeFader(idx, defaultFaderVal);
                }
              };

              const handleFaderContainerClick = (e: React.MouseEvent) => {
                if (isMidiLearnActive) {
                  e.stopPropagation();
                  onSelectLearnTarget?.({ type: "fader", index: idx });
                }
              };

              return (
                <div
                  key={idx}
                  onClick={handleFaderContainerClick}
                  onDoubleClick={handleFaderReset}
                  className="flex-1 flex flex-col items-center cursor-pointer transition-all rounded-lg p-0.5"
                >
                  <div
                    onPointerDown={(e) => {
                      if (!isMidiLearnActive) handleFaderPointerDown(idx, e);
                    }}
                    className={`w-full h-28 bg-[#08080a] rounded-lg relative flex items-center justify-center p-0.5 cursor-ns-resize transition shadow-[inset_0_2px_6px_rgba(0,0,0,0.9)] overflow-hidden group/fader ${
                      isTargetSelected
                        ? "border-2 border-amber-400 shadow-[0_0_18px_rgba(245,158,11,0.9)] ring-2 ring-amber-400/50 animate-pulse"
                        : isMidiLearnActive
                        ? "border-2 border-amber-500/60 shadow-[0_0_12px_rgba(245,158,11,0.35)] hover:border-amber-400 hover:shadow-[0_0_16px_rgba(245,158,11,0.6)]"
                        : isMaster
                        ? "border-2 border-cyan-400/90 shadow-[0_0_12px_rgba(34,211,238,0.35),inset_0_2px_6px_rgba(0,0,0,0.9)] hover:border-cyan-300"
                        : "border border-white/[0.08] hover:border-white/20"
                    }`}
                    title={
                      isMidiLearnActive
                        ? `MIDI Learn: Fader ${idx + 1} (CC ${mappedCc})`
                        : isMaster
                        ? `Master: ${val}`
                        : `Pad ${idx + 1}${padSample ? ` (${padSample})` : ""}: ${Math.round((val / 127) * 100)}%`
                    }
                  >
                    {/* Ranura vertical metálica del chasis (Fader Track Slot) */}
                    <div className="absolute left-1/2 -translate-x-1/2 w-[3px] h-[calc(100%-8px)] bg-black rounded-full border border-white/[0.06] shadow-inner" />

                    {/* Relleno sutil del nivel de señal en la ranura */}
                    <div
                      className={`absolute left-1/2 -translate-x-1/2 w-[2px] bottom-1 rounded-full transition-all duration-75 ${
                        isMaster
                          ? "bg-cyan-400/80 shadow-[0_0_6px_rgba(34,211,238,0.6)]"
                          : `${colorTheme.dot} opacity-60`
                      }`}
                      style={{ height: `${Math.max(0, heightPct * 0.78)}%` }}
                    />

                    {/* Fader Cap (Perilla Deslizante Táctil de Hardware) */}
                    <div
                      className="absolute left-1/2 -translate-x-1/2 w-[88%] h-4 rounded-sm bg-gradient-to-b from-[#383a42] via-[#22242a] to-[#141518] border border-white/20 shadow-[0_3px_6px_rgba(0,0,0,0.9),inset_0_1px_0_rgba(255,255,255,0.35)] flex items-center justify-center transition-all duration-75 pointer-events-none group-hover/fader:border-white/40"
                      style={{
                        bottom: `calc(${heightPct}% * 0.76 + 2px)`,
                      }}
                    >
                      {/* Estría táctil y línea indicadora central */}
                      <div className={`w-[75%] h-[2px] rounded-full ${isMaster ? "bg-cyan-400 shadow-[0_0_6px_#22d3ee]" : colorTheme.line}`} />
                    </div>
                  </div>

                  {/* Indicador de Valor numérico limpio */}
                  <span
                    className={`text-[9.5px] font-mono mt-1.5 tabular-nums transition-colors ${
                      isTargetSelected
                        ? "text-amber-300 font-bold"
                        : isMidiLearnActive
                        ? "text-amber-400/80 font-medium"
                        : isMaster
                        ? "text-cyan-300 font-bold"
                        : "text-zinc-300 font-medium"
                    }`}
                  >
                    {val}
                  </span>
                </div>
              );
            })}
          </div>
        </div>



        {/* Sección Knobs Matrix & FX Rack (K1 a K8) con Pantalla OLED de Instrumento/Preset */}
        <div className="flex-1 min-w-0 pr-0 lg:pr-4 pb-3 lg:pb-0 border-b lg:border-b-0 lg:border-r border-white/[0.08] flex flex-col justify-between">
          {/* Pantalla OLED de Selección de Instrumento Melódico y Patch / Preset */}
          <div className="mb-2 p-1 bg-[#040406] border border-white/10 rounded-lg shadow-[inset_0_2px_4px_rgba(0,0,0,0.9),0_1px_0_rgba(255,255,255,0.05)] flex items-center justify-between gap-2 flex-wrap">
            {/* Selector de Instrumento Melódico (Coherente 1:1 con PATCH) */}
            <div className="flex-1 min-w-[170px] h-7 px-2 bg-[#0a0c10] border border-white/10 hover:border-white/25 rounded-md flex items-center gap-1.5 transition-all shadow-[inset_0_1px_2px_rgba(0,0,0,0.8)]">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_#10b981] shrink-0" />
              <span className="text-[9.5px] font-mono uppercase font-bold tracking-wider text-zinc-400 shrink-0">
                INST:
              </span>
              <select
                value={currentSoundfont?.filename || ""}
                onChange={(e) => {
                  const val = e.target.value;
                  e.target.blur();
                  if (val && onSelectSoundfont) {
                    onSelectSoundfont(val);
                  }
                }}
                className="bg-transparent text-emerald-300 font-mono text-[10.5px] font-bold focus:outline-none cursor-pointer w-full truncate pr-1"
                title="Cambiar Instrumento"
              >
                {availableSoundfonts.length > 0 ? (
                  availableSoundfonts.map((sf) => (
                    <option
                      key={sf.filename}
                      value={sf.filename}
                      className="bg-zinc-950 text-zinc-200 font-mono text-xs"
                    >
                      {sf.displayName.toUpperCase()}
                    </option>
                  ))
                ) : (
                  <option value={currentSoundfont?.filename || ""} className="bg-zinc-950 text-zinc-200 font-mono text-xs">
                    {currentSoundfont ? currentSoundfont.displayName.toUpperCase() : "CARGANDO..."}
                  </option>
                )}
              </select>
            </div>

            {/* Selector de Preset (visible cuando el soundfont tiene más de 1 preset, ej. Mellotron) */}
            {currentSoundfont && currentSoundfont.presets.length > 1 && (
              <div className="h-7 px-2 bg-[#0a0c10] border border-white/10 hover:border-white/25 rounded-md flex items-center gap-1.5 transition-all shadow-[inset_0_1px_2px_rgba(0,0,0,0.8)]">
                <span className="w-1.5 h-1.5 rounded-full bg-amber-400 shadow-[0_0_6px_#f59e0b]" />
                <span className="text-[9.5px] font-mono uppercase font-bold tracking-wider text-zinc-400 shrink-0">
                  PATCH:
                </span>
                <select
                  value={activePresetIndex}
                  onChange={(e) => {
                    const idx = parseInt(e.target.value, 10);
                    e.target.blur();
                    const preset = currentSoundfont.presets[idx];
                    const patch = preset?.patch ?? 0;
                    const bank = preset?.bank ?? 0;
                    onSelectPreset?.(patch, idx, bank);
                  }}
                  className="bg-transparent text-amber-300 font-mono text-[10.5px] font-bold focus:outline-none cursor-pointer pr-1"
                >
                  {currentSoundfont.presets.map((p, idx) => (
                    <option key={idx} value={idx} className="bg-zinc-950 text-zinc-200 font-mono text-xs">
                      {p.name.toUpperCase()}
                    </option>
                  ))}
                </select>
              </div>
            )}
          </div>
          <div className="grid grid-cols-4 sm:grid-cols-8 gap-1.5 sm:gap-2">
            {knobs.map((val, idx) => {
              const rotDeg = -135 + (val / 127) * 270;
              const mapping = knobMappings.find((m) => m.index === idx);
              const defaultLabel =
                idx < 4
                  ? ["CUTOFF", "RES", "DRIVE", "REVERB"][idx]
                  : ["D-TIME", "D-FDBK", "D-MIX", "PAN"][idx - 4];
              const activeLabel = mapping ? mapping.label : defaultLabel;
              const displayName = mapping ? mapping.displayName : activeLabel;

              // Constantes de arco circular (r = 14, circunferencia = 87.96, arco de 270° = 65.97)
              const circ = 87.96;
              const arcLength = 65.97;
              const activeDash = (val / 127) * arcLength;

              const target = mapping ? mapping.target : DEFAULT_KNOB_TARGETS[idx] || "cutoff";
              const defaultKnobVal = getKnobDefaultValue(target);
              const isFxBypassed = fxRouting === "off";

              const isKnobTargetSelected =
                isMidiLearnActive &&
                selectedLearnTarget?.type === "knob" &&
                selectedLearnTarget?.index === idx;
              const mappedKnobCc = midiLearnMapping?.knobs[idx] ?? (56 + idx);

              const handleKnobReset = (e: React.MouseEvent) => {
                e.stopPropagation();
                if (!isMidiLearnActive) {
                  onChangeKnob(idx, defaultKnobVal);
                }
              };

              const handleKnobContainerClick = (e: React.MouseEvent) => {
                if (isMidiLearnActive) {
                  e.stopPropagation();
                  onSelectLearnTarget?.({ type: "knob", index: idx });
                }
              };

              return (
                <div
                  key={idx}
                  onClick={handleKnobContainerClick}
                  onDoubleClick={handleKnobReset}
                  className={`flex flex-col items-center cursor-pointer group py-1 px-1 rounded-lg transition-all ${
                    isKnobTargetSelected
                      ? "border-2 border-amber-400 bg-amber-500/20 shadow-[0_0_18px_rgba(245,158,11,0.85)] ring-2 ring-amber-400/50 animate-pulse"
                      : isMidiLearnActive
                      ? "border border-amber-500/50 bg-amber-950/20 shadow-[0_0_10px_rgba(245,158,11,0.3)] hover:border-amber-400 hover:shadow-[0_0_14px_rgba(245,158,11,0.55)]"
                      : "border border-transparent hover:bg-white/[0.03]"
                  }`}
                  onContextMenu={(e) => {
                    if (isMidiLearnActive) return;
                    e.preventDefault();
                    e.stopPropagation();
                    const x = Math.min(e.clientX, window.innerWidth - 260);
                    const y = Math.min(e.clientY, window.innerHeight - 340);
                    setKnobContextMenu({ knobIdx: idx, x, y });
                  }}
                  title={
                    isMidiLearnActive
                      ? `MIDI Learn: Knob ${idx + 1} (CC ${mappedKnobCc})`
                      : `Knob ${idx + 1}: ${displayName} (${val})`
                  }
                >
                  {/* Knob con Anillo de Progreso Circular */}
                  <div
                    onPointerDown={(e) => handleKnobPointerDown(idx, e)}
                    className="relative w-9 h-9 flex items-center justify-center cursor-ns-resize mt-0.5"
                  >
                    {/* Anillo de Medición SVG (Arc Gauge de 270 grados) */}
                    <svg className="w-9 h-9 rotate-[135deg] pointer-events-none" viewBox="0 0 36 36">
                      {/* Pista de fondo */}
                      <circle
                        cx="18"
                        cy="18"
                        r="14"
                        fill="none"
                        stroke="#1e2026"
                        strokeWidth="2.5"
                        strokeDasharray={`${arcLength} ${circ}`}
                        strokeLinecap="round"
                      />
                      {/* Pista activa luminosa (ámbar si está activo, atenuada si FX está en bypass) */}
                      <circle
                        cx="18"
                        cy="18"
                        r="14"
                        fill="none"
                        stroke={isFxBypassed ? "#3f3f46" : "#f59e0b"}
                        strokeWidth="2.5"
                        strokeDasharray={`${activeDash} ${circ}`}
                        strokeLinecap="round"
                        className={
                          isFxBypassed
                            ? "transition-all duration-75 opacity-60"
                            : "transition-all duration-75 drop-shadow-[0_0_5px_rgba(251,191,36,0.85)]"
                        }
                      />
                    </svg>

                    {/* Perilla Física Central */}
                    <div className="absolute w-6 h-6 rounded-full bg-gradient-to-b from-[#34373e] via-[#1e2025] to-[#121316] border border-white/15 shadow-[0_2px_5px_rgba(0,0,0,0.8),inset_0_1px_0_rgba(255,255,255,0.25)] flex items-center justify-center group-hover:border-amber-400/60 transition">
                      {/* Aguja del knob */}
                      <div
                        className={`absolute w-[1.5px] h-2.5 rounded-full origin-bottom bottom-1/2 transition-all ${
                          isFxBypassed
                            ? "bg-zinc-500 shadow-none"
                            : "bg-amber-400 shadow-[0_0_6px_#fbbf24]"
                        }`}
                        style={{ transform: `rotate(${rotDeg}deg)` }}
                      />
                    </div>
                  </div>

                  {/* Etiqueta / Selector de Destino Clickable */}
                  <button
                    type="button"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      const rect = e.currentTarget.getBoundingClientRect();
                      const x = Math.min(rect.left, window.innerWidth - 260);
                      const y = Math.min(rect.bottom + 4, window.innerHeight - 340);
                      setKnobContextMenu({ knobIdx: idx, x, y });
                    }}
                    title="Reasignar modulación"
                    className={`text-[9.5px] font-mono font-bold mt-1.5 truncate max-w-[58px] text-center leading-tight tracking-wider px-1 py-0.5 rounded transition-all flex items-center justify-center gap-0.5 ${
                      isFxBypassed
                        ? "text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.04]"
                        : "text-amber-300 hover:text-amber-200 hover:bg-amber-400/10"
                    }`}
                  >
                    <span className="truncate">{activeLabel}</span>
                    <span className="text-[7.5px] opacity-70">▾</span>
                  </button>

                  <span
                    className={`text-[9.5px] font-mono mt-0.5 font-medium tabular-nums transition-colors ${
                      isKnobTargetSelected
                        ? "text-amber-300 font-bold"
                        : isMidiLearnActive
                        ? "text-amber-400/80 font-semibold"
                        : isFxBypassed
                        ? "text-zinc-600"
                        : "text-zinc-300"
                    }`}
                  >
                    {val}
                  </span>
                </div>
              );
            })}
          </div>
        </div>

        {/* Sección Performance & Routing Bay (Centrado verticalmente) */}
        <div className="w-full lg:w-28 shrink-0 flex flex-col items-center justify-center self-center my-auto">
          <PerformanceBay
            fxRouting={fxRouting}
            onChangeFxRouting={onChangeFxRouting}
            isMidiLearnActive={isMidiLearnActive}
            selectedLearnTarget={selectedLearnTarget}
            onSelectLearnTarget={onSelectLearnTarget}
            midiLearnMapping={midiLearnMapping}
          />
        </div>
      </div>

      {/* Menú Contextual Noir para Knobs (Clic Derecho) */}
      {knobContextMenu && (
        <div
          className="fixed z-50 bg-[#121214] border border-white/10 rounded-xl shadow-2xl py-1.5 w-64 text-xs font-mono text-slate-200 animate-in fade-in zoom-in-95 duration-100"
          style={{ top: knobContextMenu.y, left: knobContextMenu.x }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="px-3 py-1 text-[10px] uppercase font-bold text-slate-400 border-b border-white/10 mb-1 flex justify-between items-center">
            <span>Asignar Knob K{knobContextMenu.knobIdx + 1}</span>
            <span className="text-[9px] text-amber-400 font-semibold">
              {knobMappings.find((m) => m.index === knobContextMenu.knobIdx)?.label || ""}
            </span>
          </div>
          <div className="max-h-72 overflow-y-auto divide-y divide-white/5">
            {AVAILABLE_KNOB_TARGETS.map((t) => {
              const currentTarget = knobMappings.find((m) => m.index === knobContextMenu.knobIdx)?.target;
              const isSelected = currentTarget === t.target;
              return (
                <button
                  key={t.target}
                  type="button"
                  className={`w-full text-left px-3 py-1.5 flex items-center justify-between transition-colors ${
                    isSelected
                      ? "bg-amber-950/40 text-amber-300 font-bold"
                      : "hover:bg-white/5 hover:text-white text-slate-300"
                  }`}
                  onClick={() => {
                    const idx = knobContextMenu.knobIdx;
                    setKnobContextMenu(null);
                    onSetKnobMapping?.(idx, t.target);
                  }}
                >
                  <div className="flex flex-col">
                    <span className="text-[11px] font-semibold tracking-wide">
                      {t.label}
                    </span>
                    <span className="text-[9px] text-slate-500 font-normal">
                      {t.desc}
                    </span>
                  </div>
                  {isSelected && (
                    <Check size={12} className="text-amber-400 ml-2 shrink-0" />
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}

      {/* 2. Deck Inferior: Bahía del Teclado de 49 Teclas Empotrado en el Chasis */}
      {!hideKeyboard && (
        <div className="bg-[#050507] flex flex-col relative">
          {/* Hendidura superior de montaje del lecho de teclas (Keybed Seam) */}
          <div className="h-1 w-full bg-gradient-to-b from-black to-[#09090c] border-b border-black/90 shadow-[inset_0_2px_4px_rgba(0,0,0,0.95)] shrink-0 z-20" />

          {/* Lecho del Teclado continuo con topes laterales de chasis (Cheek Blocks) */}
          <div className="flex w-full items-stretch relative">
            {/* Cheek Block Izquierdo */}
            <div className="w-2 sm:w-3 bg-gradient-to-r from-[#0c0d10] to-[#060608] border-r border-black/90 shadow-[inset_1px_0_0_rgba(255,255,255,0.03),inset_-2px_0_4px_rgba(0,0,0,0.8)] shrink-0 z-30" />

            {/* Riel del Teclado Alargado Recto */}
            <div
              className="relative flex-1 h-44 flex overflow-x-auto overflow-y-hidden select-none bg-black"
              onMouseLeave={() => {
                // Silenciar cualquier nota colgada si el cursor sale del riel
                for (const k of keys) {
                  if (k.isActive) onReleaseNote(k.note);
                }
              }}
            >
              {keys.map((k) => {
                const isCKey = k.note % 12 === 0;
                const octaveNum = Math.floor(k.note / 12) - 2; // 36 = C1, 60 = C3, etc.
                const isMiddleC = k.note === 60;

                if (k.isBlack) {
                  const qwertyLabel = qwertyLabels.get(k.note);
                  return (
                    <div
                      key={k.note}
                      onMouseDown={(e) => {
                        if (k.isOutOfRange) return;
                        e.stopPropagation();
                        onTriggerNote(k.note, 110);
                      }}
                      onMouseUp={(e) => {
                        if (k.isOutOfRange) return;
                        e.stopPropagation();
                        onReleaseNote(k.note);
                      }}
                      onMouseEnter={(e) => {
                        if (k.isOutOfRange) return;
                        if (e.buttons === 1) onTriggerNote(k.note, 110);
                      }}
                      onMouseLeave={(e) => {
                        if (k.isOutOfRange) return;
                        if (e.buttons === 1) onReleaseNote(k.note);
                      }}
                      className={`w-[22px] h-[106px] -mx-[11px] z-20 rounded-b-[2px] transition-all duration-75 relative ${
                        k.isOutOfRange
                          ? "bg-zinc-950 border-x border-b border-black opacity-20 cursor-not-allowed shadow-none"
                          : k.isActive
                          ? "bg-gradient-to-b from-cyan-400 via-cyan-500 to-cyan-600 border-x border-cyan-400 border-b-[4px] border-b-cyan-700 shadow-[0_0_20px_rgba(6,182,212,0.95),inset_0_4px_6px_rgba(0,0,0,0.4)] cursor-pointer"
                          : "bg-gradient-to-b from-[#18191c] via-[#121316] to-[#08080a] border-x border-[#060608] border-b-[4px] border-b-[#020203] shadow-[2px_6px_10px_rgba(0,0,0,0.95),-2px_6px_10px_rgba(0,0,0,0.95)] hover:bg-[#1f2026] cursor-pointer"
                      }`}
                      style={{ flexShrink: 0 }}
                    >
                      {qwertyLabel && !k.isOutOfRange && (
                        <span
                          className={`absolute bottom-1.5 left-1/2 -translate-x-1/2 font-mono text-[9px] font-bold tracking-tight select-none pointer-events-none transition-colors ${
                            k.isActive ? "text-cyan-950 font-black" : "text-zinc-400/90"
                          }`}
                        >
                          {qwertyLabel}
                        </span>
                      )}
                    </div>
                  );
                }

                const qwertyLabel = qwertyLabels.get(k.note);
                return (
                  <div
                    key={k.note}
                    onMouseDown={() => {
                      if (k.isOutOfRange) return;
                      onTriggerNote(k.note, 100);
                    }}
                    onMouseUp={() => {
                      if (k.isOutOfRange) return;
                      onReleaseNote(k.note);
                    }}
                    onMouseEnter={(e) => {
                      if (k.isOutOfRange) return;
                      if (e.buttons === 1) onTriggerNote(k.note, 100);
                    }}
                    onMouseLeave={(e) => {
                      if (k.isOutOfRange) return;
                      if (e.buttons === 1) onReleaseNote(k.note);
                    }}
                    className={`flex-1 min-w-[20px] h-full rounded-b-[4px] transition-all duration-75 relative z-10 ${
                      k.isOutOfRange
                        ? "bg-zinc-900/60 border-r border-zinc-950 opacity-20 cursor-not-allowed shadow-none"
                        : k.isActive
                        ? "bg-gradient-to-b from-cyan-300 via-cyan-400 to-cyan-500 text-black border-r border-cyan-300 shadow-[inset_0_4px_8px_rgba(0,0,0,0.25),inset_0_-2px_0_#0891b2,0_0_18px_rgba(6,182,212,0.9)] cursor-pointer"
                        : "bg-gradient-to-b from-[#eef1f5] via-[#fafbfc] to-[#dce2ea] border-r border-black/30 shadow-[inset_0_-5px_0_#cbd5e1,0_2px_4px_rgba(0,0,0,0.3)] hover:brightness-105 cursor-pointer"
                    }`}
                  >
                    {/* Marcador de Octava en teclas C (C1 a C5) con grabado láser */}
                    {isCKey && (
                      <span
                        className={`absolute left-1/2 -translate-x-1/2 font-mono font-bold tracking-tight select-none pointer-events-none transition-all ${
                          qwertyLabel && !k.isOutOfRange ? "bottom-7 text-[8.5px]" : "bottom-2.5 text-[9.5px]"
                        } ${
                          k.isActive
                            ? "text-black font-extrabold"
                            : isMiddleC
                            ? "text-cyan-700 font-extrabold drop-shadow-[0_1px_0_rgba(255,255,255,0.9)]"
                            : "text-zinc-500 font-bold drop-shadow-[0_1px_0_rgba(255,255,255,0.9)]"
                        }`}
                      >
                        C{octaveNum}
                      </span>
                    )}

                    {/* Serigrafía de Letra QWERTY */}
                    {qwertyLabel && !k.isOutOfRange && (
                      <span
                        className={`absolute bottom-2.5 left-1/2 -translate-x-1/2 font-mono text-[10px] font-bold tracking-tight select-none pointer-events-none transition-colors ${
                          k.isActive
                            ? "text-black font-black"
                            : "text-zinc-600/90 drop-shadow-[0_1px_0_rgba(255,255,255,0.7)]"
                        }`}
                      >
                        {qwertyLabel}
                      </span>
                    )}
                  </div>
                );
              })}
            </div>

            {/* Cheek Block Derecho */}
            <div className="w-2 sm:w-3 bg-gradient-to-l from-[#0c0d10] to-[#060608] border-l border-black/90 shadow-[inset_-1px_0_0_rgba(255,255,255,0.03),inset_2px_0_4px_rgba(0,0,0,0.8)] shrink-0 z-30" />
          </div>
        </div>
      )}

      {/* Bahía Acoplada de Expansión / Secuenciador y Docking Footer */}
      <div className="relative z-40">
        {children}
      </div>
    </div>
  );
};
