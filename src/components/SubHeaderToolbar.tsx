import React from "react";
import { Volume2, VolumeX, Activity, Radio } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { AudioStatusInfo } from "./Header";

interface SubHeaderToolbarProps {
  connected: boolean;
  activePortName: string;
  audioStatus: AudioStatusInfo | null;
  masterGain: number;
  onMasterGainChange: (val: number) => void;
}

export const SubHeaderToolbar: React.FC<SubHeaderToolbarProps> = ({
  connected,
  activePortName,
  audioStatus,
  masterGain,
  onMasterGainChange,
}) => {
  const isRunning = audioStatus ? (audioStatus.isRunning ?? audioStatus.is_running ?? true) : true;
  const sampleRate = audioStatus?.sampleRate ?? audioStatus?.sample_rate ?? 48000;
  const deviceName = audioStatus?.deviceName ?? audioStatus?.device_name ?? "WASAPI Output";

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = parseFloat(e.target.value);
    onMasterGainChange(val);
    invoke("set_master_volume", { gain: val }).catch(console.error);
  };

  return (
    <div className="h-11 bg-[#060608]/50 backdrop-blur-md border-b border-white/[0.06] px-4 md:px-6 flex items-center justify-center z-20 shadow-[0_2px_15px_rgba(0,0,0,0.4)] w-full transition-all">
      <div className="studio-console flex items-center justify-between text-xs font-mono">
        {/* Lado Izquierdo: Monitores de Conexión de Hardware (MIDI & WASAPI Audio) */}
        <div className="flex items-center gap-4">
          {/* Monitor MIDI limpio sin recuadro */}
          <div className="flex items-center space-x-1.5 text-[11px] font-mono">
            <Radio
              size={12}
              className={connected ? "text-emerald-400 animate-pulse" : "text-amber-500"}
            />
            <span className="text-zinc-500">MIDI:</span>
            <span className={connected ? "text-emerald-400 font-semibold" : "text-amber-400 font-medium"}>
              {connected ? activePortName || "Impact LX49+" : "Desconectado"}
            </span>
          </div>

          <span className="text-zinc-700">|</span>

          {/* Monitor WASAPI limpio sin recuadro */}
          <div
            className="flex items-center space-x-1.5 text-[11px] font-mono text-zinc-300"
            title={deviceName}
          >
            <Activity
              size={12}
              className={isRunning ? "text-emerald-400" : "text-amber-400 animate-pulse"}
            />
            <span>
              {isRunning ? `WASAPI ${Math.round(sampleRate / 1000)}kHz` : "Reconectando..."}
            </span>
            <span className="text-zinc-600">•</span>
            <span className={isRunning ? "text-emerald-400 font-medium" : "text-amber-400 font-medium"}>
              {isRunning ? "≤ 5.3 ms" : "Buscando"}
            </span>
          </div>
        </div>

        {/* Lado Derecho: Master Volume Slider */}
        <div className="flex items-center space-x-2 bg-white/[0.02] px-2.5 py-1 rounded-lg border border-white/[0.06]">
          {masterGain > 0 ? (
            <Volume2 size={13} className="text-emerald-400" />
          ) : (
            <VolumeX size={13} className="text-zinc-600" />
          )}
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={masterGain}
            onChange={handleVolumeChange}
            className="w-20 h-1 bg-white/10 rounded-lg appearance-none cursor-pointer accent-emerald-400"
          />
          <span className="text-[10.5px] font-mono text-zinc-300 w-8 text-right font-bold">
            {Math.round(masterGain * 100)}%
          </span>
        </div>
      </div>
    </div>
  );
};
