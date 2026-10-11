import React, { useEffect, useState } from "react";
import { X, Speaker, Cpu, RefreshCw, CheckCircle2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { AudioDeviceInfo, MidiPortInfo } from "../types/midi";

interface AudioSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  ports: MidiPortInfo[];
  onRefreshMidi: () => void;
  onSelectPort: (index: number) => void;
}

export const AudioSettingsModal: React.FC<AudioSettingsModalProps> = ({
  isOpen,
  onClose,
  ports,
  onRefreshMidi,
  onSelectPort,
}) => {
  const [audioDevices, setAudioDevices] = useState<AudioDeviceInfo[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string>("");
  const [bufferSize, setBufferSize] = useState<number>(256);
  const [sampleRate, setSampleRate] = useState<number>(48000);
  const [audioHost, setAudioHost] = useState<"WASAPI" | "ASIO">("WASAPI");

  useEffect(() => {
    if (isOpen) {
      onRefreshMidi();
      invoke<AudioDeviceInfo[]>("list_audio_devices")
        .then((devs) => {
          setAudioDevices(devs);
          const def = devs.find((d) => d.isDefault);
          if (def) setSelectedDevice(def.id);
        })
        .catch(console.error);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-md flex items-center justify-center p-4">
      <div className="bg-zinc-950/80 backdrop-blur-2xl border border-white/[0.08] w-full max-w-xl rounded-2xl shadow-2xl overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-150">
        {/* Header */}
        <div className="px-6 py-4 bg-white/[0.02] border-b border-white/[0.08] flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <Cpu className="text-emerald-400" size={18} />
            <h2 className="text-sm font-bold tracking-wide uppercase text-zinc-100">
              Configuración de Audio y Hardware
            </h2>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-white/[0.06] transition"
          >
            <X size={18} />
          </button>
        </div>

        {/* Body */}
        <div className="p-6 space-y-6 text-xs font-mono">
          {/* Driver Host Selector */}
          <div>
            <label className="text-slate-400 block mb-2 font-semibold">
              SISTEMA DE AUDIO (AUDIO HOST)
            </label>
            <div className="grid grid-cols-2 gap-3">
              <button
                type="button"
                onClick={() => setAudioHost("WASAPI")}
                className={`py-2 px-3 rounded-xl border flex items-center justify-between transition ${
                  audioHost === "WASAPI"
                    ? "bg-studio-800 border-studio-cyan text-white shadow-[0_0_10px_rgba(0,229,255,0.2)]"
                    : "bg-studio-950 border-studio-800 text-slate-400 hover:border-studio-700"
                }`}
              >
                <span>WASAPI (Windows Nativo)</span>
                {audioHost === "WASAPI" && <CheckCircle2 size={14} className="text-studio-cyan" />}
              </button>
              <button
                type="button"
                onClick={() => setAudioHost("ASIO")}
                className={`py-2 px-3 rounded-xl border flex items-center justify-between transition ${
                  audioHost === "ASIO"
                    ? "bg-studio-800 border-studio-accent text-white shadow-[0_0_10px_rgba(255,62,62,0.2)]"
                    : "bg-studio-950 border-studio-800 text-slate-400 hover:border-studio-700"
                }`}
              >
                <span>ASIO (Interfaz Externa)</span>
                {audioHost === "ASIO" && <CheckCircle2 size={14} className="text-studio-accent" />}
              </button>
            </div>
          </div>

          {/* Audio Output Device */}
          <div>
            <label className="text-slate-400 block mb-2 font-semibold">
              DISPOSITIVO DE SALIDA DE AUDIO
            </label>
            <div className="flex items-center space-x-2">
              <div className="relative flex-1">
                <Speaker className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={14} />
                <select
                  value={selectedDevice}
                  onChange={(e) => setSelectedDevice(e.target.value)}
                  className="w-full bg-studio-950 border border-studio-800 rounded-xl py-2 pl-9 pr-3 text-slate-200 focus:outline-none focus:border-studio-cyan"
                >
                  {audioDevices.map((d) => (
                    <option key={d.id} value={d.id}>
                      {d.name} {d.isDefault ? "(Predeterminado)" : ""}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          </div>

          {/* Buffer Size & Sample Rate */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="text-slate-400 block mb-2 font-semibold">
                TAMAÑO DE BUFFER (LATENCIA)
              </label>
              <select
                value={bufferSize}
                onChange={(e) => setBufferSize(Number(e.target.value))}
                className="w-full bg-studio-950 border border-studio-800 rounded-xl p-2 text-slate-200 focus:outline-none focus:border-studio-cyan"
              >
                <option value={64}>64 samples (1.3 ms) - Ultra Low</option>
                <option value={128}>128 samples (2.6 ms) - Rápido</option>
                <option value={256}>256 samples (5.3 ms) - Balanceado</option>
                <option value={512}>512 samples (10.6 ms) - Seguro</option>
              </select>
            </div>

            <div>
              <label className="text-slate-400 block mb-2 font-semibold">
                SAMPLE RATE
              </label>
              <select
                value={sampleRate}
                onChange={(e) => setSampleRate(Number(e.target.value))}
                className="w-full bg-studio-950 border border-studio-800 rounded-xl p-2 text-slate-200 focus:outline-none focus:border-studio-cyan"
              >
                <option value={44100}>44.1 kHz (CD Audio)</option>
                <option value={48000}>48.0 kHz (Estándar Studio)</option>
                <option value={96000}>96.0 kHz (Hi-Res)</option>
              </select>
            </div>
          </div>

          {/* MIDI Ports Detected with Interactive Selector */}
          <div className="pt-4 border-t border-studio-800">
            <div className="flex items-center justify-between mb-2">
              <label className="text-slate-400 font-semibold uppercase tracking-wider">
                Selección de Puerto MIDI
              </label>
              <button
                onClick={onRefreshMidi}
                className="text-studio-cyan hover:underline flex items-center gap-1 text-[11px]"
              >
                <RefreshCw size={11} /> Re-escanear
              </button>
            </div>
            <div className="space-y-2">
              {ports.map((p) => (
                <div
                  key={p.index}
                  className="px-3 py-2 rounded-lg bg-studio-950 border border-studio-800 flex items-center justify-between"
                >
                  <div className="flex items-center space-x-2">
                    <span className="text-slate-400 font-mono text-[11px]">[{p.index}]</span>
                    <span className="text-slate-200 font-medium">{p.name}</span>
                    {p.isDefault && (
                      <span className="text-[9px] bg-studio-800 text-studio-cyan px-1.5 py-0.5 rounded border border-studio-700">
                        Recomendado
                      </span>
                    )}
                  </div>
                  <div>
                    {p.isConnected ? (
                      <span className="text-emerald-400 text-[11px] font-semibold bg-emerald-950/70 border border-emerald-800/80 px-2.5 py-1 rounded-lg flex items-center gap-1.5 shadow-[0_0_8px_rgba(16,185,129,0.2)]">
                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                        Conectado
                      </span>
                    ) : (
                      <button
                        onClick={() => onSelectPort(p.index)}
                        className="px-3 py-1 rounded-lg bg-studio-800 hover:bg-studio-700 text-slate-300 hover:text-white border border-studio-700 text-[11px] font-medium transition flex items-center gap-1"
                      >
                        Conectar
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-6 py-3 bg-studio-850 border-t border-studio-800 flex justify-end">
          <button
            onClick={onClose}
            className="px-4 py-1.5 rounded-lg bg-studio-cyan hover:bg-cyan-400 text-black font-semibold text-xs transition"
          >
            Listo
          </button>
        </div>
      </div>
    </div>
  );
};
