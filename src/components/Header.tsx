import React, { useState, useRef, useEffect } from "react";
import { Sliders, Volume2, VolumeX, RotateCcw, Radio, Activity, Keyboard } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { NormalizedMidiEvent } from "../types/midi";
import { MidiDinIcon, MidiStreamPopover } from "./MidiStreamPopover";

export interface AudioStatusInfo {
  host?: string;
  deviceName?: string;
  sampleRate?: number;
  bufferSize?: number;
  isRunning?: boolean;
  device_name?: string;
  sample_rate?: number;
  buffer_size?: number;
  is_running?: boolean;
  driver?: string;
  channels?: number;
}

interface HeaderProps {
  onOpenSettings: () => void;
  onOpenInstrumentSelect?: () => void;
  midiEvents?: NormalizedMidiEvent[];
  onClearMidiEvents?: () => void;
  connected?: boolean;
  activePortName?: string;
  audioStatus?: AudioStatusInfo | null;
  masterGain?: number;
  onMasterGainChange?: (val: number) => void;
  isMidiLearnActive?: boolean;
  onToggleMidiLearn?: () => void;
  qwertyEnabled?: boolean;
  onToggleQwerty?: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  onOpenSettings,
  onOpenInstrumentSelect,
  midiEvents = [],
  onClearMidiEvents,
  connected = false,
  activePortName = "",
  audioStatus = null,
  masterGain = 0.8,
  onMasterGainChange,
  isMidiLearnActive = false,
  onToggleMidiLearn,
  qwertyEnabled = false,
  onToggleQwerty,
}) => {
  const [isMidiPopoverOpen, setIsMidiPopoverOpen] = useState(false);
  const [isVolumeOpen, setIsVolumeOpen] = useState(false);

  const midiTriggerRef = useRef<HTMLButtonElement>(null);
  const volumeTriggerRef = useRef<HTMLButtonElement>(null);
  const volumePopoverRef = useRef<HTMLDivElement>(null);
  const lastNonZeroGainRef = useRef(masterGain > 0 ? masterGain : 0.8);

  const isRunning = audioStatus ? (audioStatus.isRunning ?? audioStatus.is_running ?? true) : true;
  const sampleRate = audioStatus?.sampleRate ?? audioStatus?.sample_rate ?? 48000;
  const bufferSize = audioStatus?.bufferSize ?? audioStatus?.buffer_size ?? 256;
  const deviceName = audioStatus?.deviceName ?? audioStatus?.device_name ?? "Salida de Audio";
  const driver = audioStatus?.driver ?? "Audio";
  const channels = audioStatus?.channels ?? 2;

  const realLatencyMs = (bufferSize && sampleRate && sampleRate > 0)
    ? ((bufferSize / sampleRate) * 1000).toFixed(1)
    : "5.3";

  // Cierre de popover de volumen al hacer clic afuera
  useEffect(() => {
    if (!isVolumeOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (volumeTriggerRef.current && volumeTriggerRef.current.contains(target)) {
        return;
      }
      if (volumePopoverRef.current && !volumePopoverRef.current.contains(target)) {
        setIsVolumeOpen(false);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsVolumeOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isVolumeOpen]);

  const handleVolumeChange = (val: number) => {
    if (val > 0) lastNonZeroGainRef.current = val;
    onMasterGainChange?.(val);
    invoke("set_master_volume", { gain: val }).catch(console.error);
  };

  const handleToggleMute = () => {
    if (masterGain > 0) {
      lastNonZeroGainRef.current = masterGain;
      handleVolumeChange(0);
    } else {
      handleVolumeChange(lastNonZeroGainRef.current || 0.8);
    }
  };

  const handleResetVolume = () => {
    handleVolumeChange(0.8); // 80% nominal
  };

  return (
    <header className="h-14 bg-[#08080a]/75 backdrop-blur-md border-b border-white/[0.08] px-4 md:px-6 flex items-center justify-center z-30 shadow-[0_4px_30px_rgba(0,0,0,0.5),inset_0_1px_0_0_rgba(255,255,255,0.06)] w-full transition-all">
      <div className="studio-console flex items-center justify-between">
        {/* Lado Izquierdo: Brand Logo + Monitores Hardware Inline */}
        <div className="flex items-center space-x-3 sm:space-x-4">
          <button
            type="button"
            onClick={onOpenInstrumentSelect}
            className="text-sm font-black tracking-widest text-white flex items-center select-none hover:opacity-90 transition cursor-pointer group focus:outline-none shrink-0"
            style={{ fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif' }}
            title="Instrumentos"
          >
            <h1>
              IMPACT<span className="text-emerald-400 group-hover:drop-shadow-[0_0_10px_rgba(16,185,129,0.7)] transition-all">SAMPLER</span>
            </h1>
          </button>

          <div className="h-4 w-[1px] bg-white/10 hidden sm:block shrink-0" />

          {/* Monitores Inline Telemetría (MIDI & WASAPI) */}
          <div className="hidden sm:flex items-center space-x-3 text-[11px] font-mono">
            {/* Monitor MIDI */}
            <div className="flex items-center space-x-1.5">
              <Radio
                size={12}
                className={connected ? "text-emerald-400 animate-pulse" : "text-amber-500"}
              />
              <span className="text-zinc-500 hidden md:inline">MIDI:</span>
              <span className={connected ? "text-emerald-400 font-semibold" : "text-amber-400 font-medium"}>
                {connected ? activePortName || "Impact LX49+" : "Desconectado"}
              </span>
            </div>

            <span className="text-zinc-700 hidden md:inline">•</span>

            {/* Monitor Audio Hardware */}
            <div
              className="flex items-center space-x-1.5 text-zinc-300"
              title={`${deviceName} (${driver}) · ${channels} canales · Buffer: ${bufferSize} · ${sampleRate}Hz`}
            >
              <Activity
                size={12}
                className={isRunning ? "text-emerald-400" : "text-amber-400 animate-pulse"}
              />
              <span className="max-w-[120px] md:max-w-[160px] truncate font-medium text-zinc-200">
                {isRunning ? deviceName : "Reconectando..."}
              </span>
              {isRunning ? (
                <>
                  <span className="text-zinc-600 hidden lg:inline">•</span>
                  <span className="text-zinc-400 hidden lg:inline">
                    {Math.round(sampleRate / 1000)}kHz
                  </span>
                  <span className="text-zinc-600">•</span>
                  <span className="text-emerald-400 font-medium">
                    ~{realLatencyMs} ms
                  </span>
                </>
              ) : (
                <>
                  <span className="text-zinc-600">•</span>
                  <span className="text-amber-400 font-medium">
                    Buscando
                  </span>
                </>
              )}
            </div>
          </div>
        </div>

        {/* Lado Derecho: Live MIDI DIN + Botón Settings + Master Volume Vertical */}
        <div className="flex items-center space-x-2">
          {/* Live MIDI Stream Popover Trigger */}
          <div className="relative">
            <button
              ref={midiTriggerRef}
              type="button"
              onClick={() => setIsMidiPopoverOpen((prev) => !prev)}
              className={`p-2 rounded-xl border transition flex items-center justify-center shadow-sm cursor-pointer ${
                isMidiPopoverOpen
                  ? "bg-emerald-500/20 text-emerald-300 border-emerald-500/50 shadow-[0_0_12px_rgba(16,185,129,0.3)]"
                  : "bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border-white/[0.08]"
              }`}
              title="Monitor MIDI"
            >
              <MidiDinIcon size={16} />
            </button>
            <MidiStreamPopover
              isOpen={isMidiPopoverOpen}
              onClose={() => setIsMidiPopoverOpen(false)}
              triggerRef={midiTriggerRef}
              events={midiEvents}
              onClear={onClearMidiEvents}
            />
          </div>

          {/* QWERTY Virtual Keyboard Toggle Button */}
          <button
            type="button"
            onClick={(e) => {
              e.currentTarget.blur();
              onToggleQwerty?.();
            }}
            className={`p-2 rounded-xl border transition flex items-center justify-center cursor-pointer select-none ${
              qwertyEnabled
                ? "bg-emerald-500/20 text-emerald-400 border-emerald-500/50 shadow-[0_0_12px_rgba(16,185,129,0.3)]"
                : "bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-zinc-200 border-white/[0.08]"
            }`}
            title="Teclado de computadora QWERTY (ON/OFF)"
          >
            <Keyboard
              size={16}
              className={qwertyEnabled ? "drop-shadow-[0_0_6px_rgba(52,211,153,0.8)]" : ""}
            />
          </button>

          {/* MIDI Learn Toggle Button */}
          <button
            type="button"
            onClick={onToggleMidiLearn}
            className={`px-2.5 py-1.5 rounded-xl border transition flex items-center gap-1.5 cursor-pointer text-xs font-mono select-none ${
              isMidiLearnActive
                ? "bg-amber-500/20 text-amber-300 border-amber-400 shadow-[0_0_15px_rgba(245,158,11,0.5)] font-bold"
                : "bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border-white/[0.08]"
            }`}
            title={isMidiLearnActive ? "Desactivar MIDI Learn" : "MIDI Learn"}
          >
            <span
              className={`w-2 h-2 rounded-full transition-all ${
                isMidiLearnActive ? "bg-amber-400 shadow-[0_0_8px_#f59e0b] animate-pulse" : "bg-zinc-600"
              }`}
            />
            <span className="text-[11px] font-mono tracking-wider font-semibold">
              MIDI LEARN
            </span>
          </button>

          {/* Settings Button */}
          <button
            type="button"
            onClick={onOpenSettings}
            className="p-2 rounded-xl bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] transition shadow-sm cursor-pointer"
            title="Ajustes de Audio / MIDI"
          >
            <Sliders size={16} />
          </button>

          {/* Master Volume Control Vertical */}
          <div className="relative">
            <button
              ref={volumeTriggerRef}
              type="button"
              onClick={() => setIsVolumeOpen((prev) => !prev)}
              className={`px-2.5 py-1.5 rounded-xl border transition flex items-center gap-1.5 cursor-pointer text-xs font-mono select-none ${
                isVolumeOpen
                  ? "bg-emerald-500/20 text-emerald-300 border-emerald-500/50 shadow-[0_0_12px_rgba(16,185,129,0.3)] font-semibold"
                  : "bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border-white/[0.08]"
              }`}
              title={`Volumen Master: ${Math.round(masterGain * 100)}%`}
            >
              {masterGain > 0 ? (
                <Volume2 size={15} className="text-emerald-400 shrink-0" />
              ) : (
                <VolumeX size={15} className="text-zinc-500 shrink-0" />
              )}
              <span className="text-[11px] font-mono tabular-nums text-zinc-200 font-medium">
                {Math.round(masterGain * 100)}%
              </span>
            </button>

            {/* Popover flotante del Fader Vertical Noir-Tech */}
            {isVolumeOpen && (
              <div
                ref={volumePopoverRef}
                className="absolute right-0 top-full mt-2 z-50 bg-black/95 backdrop-blur-2xl border border-white/[0.12] rounded-2xl shadow-[0_25px_60px_rgba(0,0,0,0.98),inset_0_1px_0_rgba(255,255,255,0.08)] p-3 flex flex-col items-center gap-2.5 w-28 animate-in fade-in zoom-in-95 duration-100"
              >
                <div className="flex items-center justify-between w-full border-b border-white/[0.08] pb-1.5">
                  <span className="text-[9.5px] uppercase font-bold tracking-wider text-zinc-400 font-sans">
                    MASTER
                  </span>
                  <span className="text-[11px] font-mono font-bold tabular-nums text-emerald-400">
                    {Math.round(masterGain * 100)}%
                  </span>
                </div>

                {/* Riel Vertical Fader */}
                <div className="h-32 flex items-center justify-center py-1">
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    value={masterGain}
                    onChange={(e) => handleVolumeChange(parseFloat(e.target.value))}
                    onDoubleClick={handleResetVolume}
                    style={{
                      writingMode: "vertical-lr",
                      direction: "rtl",
                      WebkitAppearance: "slider-vertical",
                    }}
                    className="h-28 w-4 accent-emerald-400 cursor-pointer"
                    title="Doble clic: 80%"
                  />
                </div>

                {/* Acciones de pie: Silencio (Mute) y Reset (80%) */}
                <div className="flex items-center justify-between w-full pt-1.5 border-t border-white/[0.08] gap-1">
                  <button
                    type="button"
                    onClick={handleToggleMute}
                    className={`flex-1 py-1 rounded-lg border text-[10px] font-sans font-medium flex items-center justify-center transition cursor-pointer ${
                      masterGain === 0
                        ? "bg-red-500/20 text-red-400 border-red-500/40"
                        : "bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-zinc-200 border-white/[0.06]"
                    }`}
                    title={masterGain === 0 ? "Unmute" : "Mute"}
                  >
                    {masterGain === 0 ? "Muted" : "Mute"}
                  </button>
                  <button
                    type="button"
                    onClick={handleResetVolume}
                    className="p-1 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white border border-white/[0.06] transition cursor-pointer"
                    title="Restablecer al 80%"
                  >
                    <RotateCcw size={12} />
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </header>
  );
};
