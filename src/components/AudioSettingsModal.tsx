import React, { useEffect, useState, useRef } from "react";
import { X, Speaker, Cpu, RefreshCw, CheckCircle2, ChevronDown, Check, RotateCcw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { AudioDeviceInfo, MidiPortInfo } from "../types/midi";
import { AudioStatusInfo } from "./Header";

interface CustomSelectOption<T> {
  value: T;
  label: string;
  subLabel?: string;
  isDefault?: boolean;
}

interface CustomSelectProps<T> {
  value: T;
  onChange: (val: T) => void;
  options: CustomSelectOption<T>[];
  icon?: React.ReactNode;
  placeholder?: string;
}

function CustomSelect<T extends string | number>({
  value,
  onChange,
  options,
  icon,
  placeholder = "Seleccionar...",
}: CustomSelectProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen) return;
    const handleOutsideClick = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsOpen(false);
    };
    document.addEventListener("mousedown", handleOutsideClick);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleOutsideClick);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen]);

  const selectedOption = options.find((o) => o.value === value);

  return (
    <div ref={containerRef} className="relative w-full">
      <button
        type="button"
        onClick={() => setIsOpen((prev) => !prev)}
        className={`w-full bg-black/60 hover:bg-black/80 border ${
          isOpen ? "border-cyan-400 shadow-[0_0_14px_rgba(6,182,212,0.25)]" : "border-white/10 hover:border-white/20"
        } rounded-xl py-2 px-3.5 flex items-center justify-between text-left transition-all cursor-pointer shadow-[inset_0_1px_2px_rgba(0,0,0,0.8)] focus:outline-none`}
      >
        <div className="flex items-center space-x-2.5 min-w-0 pr-2">
          {icon && <span className="text-zinc-400 shrink-0">{icon}</span>}
          <div className="truncate flex items-center gap-2">
            <span className="font-sans text-xs font-medium text-zinc-100 truncate">
              {selectedOption ? selectedOption.label : placeholder}
            </span>
            {selectedOption?.subLabel && (
              <span className="font-mono text-[10.5px] text-zinc-400 tabular-nums shrink-0">
                · {selectedOption.subLabel}
              </span>
            )}
            {selectedOption?.isDefault && (
              <span className="text-[9px] font-sans uppercase font-bold tracking-wider bg-white/[0.06] text-cyan-400 px-1.5 py-0.5 rounded border border-cyan-400/30 shrink-0">
                Predeterminado
              </span>
            )}
          </div>
        </div>
        <ChevronDown
          size={14}
          className={`text-zinc-400 shrink-0 transition-transform duration-200 ml-2 ${
            isOpen ? "rotate-180 text-cyan-400" : ""
          }`}
        />
      </button>

      {isOpen && (
        <div className="absolute left-0 right-0 top-full mt-1.5 z-50 bg-black/95 backdrop-blur-2xl border border-white/[0.12] rounded-xl shadow-[0_20px_50px_rgba(0,0,0,0.95),inset_0_1px_0_rgba(255,255,255,0.08)] py-1 max-h-56 overflow-y-auto animate-in fade-in zoom-in-95 duration-100 divide-y divide-white/[0.04]">
          {options.length === 0 ? (
            <div className="py-2.5 px-3 text-center text-zinc-500 font-sans text-xs select-none">
              Sin opciones disponibles
            </div>
          ) : (
            options.map((opt) => {
              const isSelected = opt.value === value;
              return (
                <button
                  key={String(opt.value)}
                  type="button"
                  onClick={() => {
                    onChange(opt.value);
                    setIsOpen(false);
                  }}
                  className={`w-full px-3.5 py-2 flex items-center justify-between text-left cursor-pointer transition text-xs ${
                    isSelected
                      ? "bg-cyan-500/10 text-cyan-200 font-medium"
                      : "text-zinc-300 hover:bg-white/[0.06] hover:text-white"
                  }`}
                >
                  <div className="flex items-center space-x-2 min-w-0">
                    <span className="font-sans font-medium truncate">{opt.label}</span>
                    {opt.subLabel && (
                      <span className="font-mono text-[10.5px] text-zinc-500 tabular-nums shrink-0">
                        · {opt.subLabel}
                      </span>
                    )}
                    {opt.isDefault && (
                      <span className="text-[9px] font-sans uppercase font-bold tracking-wider bg-white/[0.06] text-cyan-400 px-1.5 py-0.5 rounded border border-cyan-400/30 shrink-0">
                        Predeterminado
                      </span>
                    )}
                  </div>
                  {isSelected && <Check size={13} className="text-cyan-400 shrink-0 ml-2" />}
                </button>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}

interface AudioSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  ports: MidiPortInfo[];
  onRefreshMidi: () => void;
  onSelectPort: (index: number) => void;
  onResetMidiMappings?: () => void;
}

export const AudioSettingsModal: React.FC<AudioSettingsModalProps> = ({
  isOpen,
  onClose,
  ports,
  onRefreshMidi,
  onSelectPort,
  onResetMidiMappings,
}) => {
  const [audioDevices, setAudioDevices] = useState<AudioDeviceInfo[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string>("");
  const [bufferSize, setBufferSize] = useState<number>(256);
  const [sampleRate, setSampleRate] = useState<number>(48000);
  const [audioHost, setAudioHost] = useState<"WASAPI" | "ASIO">("WASAPI");
  const [isResetConfirm, setIsResetConfirm] = useState<boolean>(false);

  useEffect(() => {
    if (isOpen) {
      onRefreshMidi();
      Promise.all([
        invoke<AudioDeviceInfo[]>("list_audio_devices"),
        invoke<AudioStatusInfo>("get_audio_status"),
      ])
        .then(([devs, status]) => {
          setAudioDevices(devs);
          const activeDeviceName = status?.deviceName || status?.device_name;
          if (status?.bufferSize || status?.buffer_size) {
            setBufferSize(status.bufferSize || status.buffer_size || 256);
          }
          if (status?.sampleRate || status?.sample_rate) {
            setSampleRate(status.sampleRate || status.sample_rate || 48000);
          }

          // 1. Prioridad: Dispositivo que está actualmente en ejecución activa en WASAPI
          const activeDev = devs.find(
            (d) =>
              activeDeviceName &&
              d.name.trim().toLowerCase() === activeDeviceName.trim().toLowerCase()
          );

          // 2. Prioridad: Dispositivo predeterminado del sistema operativo
          const defaultDev = devs.find((d) => d.isDefault || (d as any).is_default);

          // 3. Fallback: Primer dispositivo encontrado
          const resolvedDev = activeDev || defaultDev || (devs.length > 0 ? devs[0] : null);

          if (resolvedDev) {
            setSelectedDevice(resolvedDev.id);
          } else {
            setSelectedDevice("");
          }
        })
        .catch(console.error);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const deviceOptions = audioDevices.map((d) => ({
    value: d.id,
    label: d.name,
    isDefault: d.isDefault ?? (d as any).is_default,
  }));

  const bufferOptions = [
    { value: 64, label: "64 samples", subLabel: "1.3 ms · Ultra Low" },
    { value: 128, label: "128 samples", subLabel: "2.6 ms · Rápido" },
    { value: 256, label: "256 samples", subLabel: "5.3 ms · Balanceado" },
    { value: 512, label: "512 samples", subLabel: "10.6 ms · Seguro" },
  ];

  const sampleRateOptions = [
    { value: 44100, label: "44.1 kHz", subLabel: "CD Audio" },
    { value: 48000, label: "48.0 kHz", subLabel: "Estándar Studio" },
    { value: 96000, label: "96.0 kHz", subLabel: "Hi-Res" },
  ];

  return (
    <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-md flex items-center justify-center p-4">
      <div className="bg-black/85 backdrop-blur-2xl border border-white/[0.12] w-full max-w-xl rounded-2xl shadow-[0_25px_80px_rgba(0,0,0,0.98),inset_0_1px_0_rgba(255,255,255,0.08)] overflow-visible flex flex-col animate-in fade-in zoom-in-95 duration-150">
        {/* Header */}
        <div className="px-6 py-4 bg-black/50 border-b border-white/[0.08] flex items-center justify-between rounded-t-2xl">
          <div className="flex items-center space-x-2">
            <Cpu className="text-emerald-400" size={18} />
            <h2 className="text-xs font-bold tracking-wider uppercase text-zinc-100 font-sans">
              Configuración de Audio y Hardware
            </h2>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-white/[0.06] transition cursor-pointer"
          >
            <X size={18} />
          </button>
        </div>

        {/* Body */}
        <div className="p-6 space-y-6 text-xs font-sans">
          {/* Driver Host Selector */}
          <div>
            <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10.5px] font-sans">
              SISTEMA DE AUDIO (AUDIO HOST)
            </label>
            <div className="grid grid-cols-2 gap-3">
              <button
                type="button"
                onClick={() => setAudioHost("WASAPI")}
                className={`py-2 px-3.5 rounded-xl border flex items-center justify-between transition cursor-pointer ${
                  audioHost === "WASAPI"
                    ? "bg-black/90 border-cyan-400 text-cyan-300 shadow-[0_0_14px_rgba(6,182,212,0.3),inset_0_1px_0_rgba(255,255,255,0.1)] font-semibold"
                    : "bg-black/50 border-white/10 text-zinc-400 hover:border-white/20 hover:text-zinc-200"
                }`}
              >
                <span className="font-sans text-xs">WASAPI (Windows Nativo)</span>
                {audioHost === "WASAPI" && <CheckCircle2 size={14} className="text-cyan-400" />}
              </button>
              <button
                type="button"
                onClick={() => setAudioHost("ASIO")}
                className={`py-2 px-3.5 rounded-xl border flex items-center justify-between transition cursor-pointer ${
                  audioHost === "ASIO"
                    ? "bg-black/90 border-amber-400 text-amber-300 shadow-[0_0_14px_rgba(245,158,11,0.3),inset_0_1px_0_rgba(255,255,255,0.1)] font-semibold"
                    : "bg-black/50 border-white/10 text-zinc-400 hover:border-white/20 hover:text-zinc-200"
                }`}
              >
                <span className="font-sans text-xs">ASIO (Interfaz Externa)</span>
                {audioHost === "ASIO" && <CheckCircle2 size={14} className="text-amber-400" />}
              </button>
            </div>
          </div>

          {/* Audio Output Device */}
          <div>
            <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10.5px] font-sans">
              DISPOSITIVO DE SALIDA DE AUDIO
            </label>
            <CustomSelect
              value={selectedDevice}
              onChange={(val) => setSelectedDevice(val)}
              options={deviceOptions}
              icon={<Speaker size={14} />}
              placeholder="Seleccionar dispositivo..."
            />
          </div>

          {/* Buffer Size & Sample Rate */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10.5px] font-sans">
                TAMAÑO DE BUFFER (LATENCIA)
              </label>
              <CustomSelect
                value={bufferSize}
                onChange={(val) => setBufferSize(val)}
                options={bufferOptions}
              />
            </div>

            <div>
              <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10.5px] font-sans">
                SAMPLE RATE
              </label>
              <CustomSelect
                value={sampleRate}
                onChange={(val) => setSampleRate(val)}
                options={sampleRateOptions}
              />
            </div>
          </div>

          {/* MIDI Ports Detected with Interactive Selector */}
          <div className="pt-4 border-t border-white/[0.08]">
            <div className="flex items-center justify-between mb-2">
              <label className="text-zinc-400 font-semibold uppercase tracking-wider text-[10.5px] font-sans">
                Selección de Puerto MIDI
              </label>
              <button
                onClick={onRefreshMidi}
                className="text-cyan-400 hover:text-cyan-300 flex items-center gap-1 text-[11px] font-sans font-medium transition cursor-pointer"
              >
                <RefreshCw size={11} /> Re-escanear
              </button>
            </div>
            <div className="space-y-2">
              {ports.map((p) => (
                <div
                  key={p.index}
                  className="px-3.5 py-2.5 rounded-xl bg-black/50 border border-white/[0.08] flex items-center justify-between hover:border-white/15 transition"
                >
                  <div className="flex items-center space-x-2.5 min-w-0 pr-2">
                    <span className="text-zinc-500 font-mono text-[11px] tabular-nums shrink-0">[{p.index}]</span>
                    <span className="text-zinc-200 font-sans text-xs font-medium truncate">{p.name}</span>
                    {p.isDefault && (
                      <span className="text-[9px] font-sans uppercase font-bold tracking-wider bg-white/[0.06] text-cyan-400 px-1.5 py-0.5 rounded border border-cyan-400/30 shadow-[0_0_8px_rgba(6,182,212,0.15)] shrink-0">
                        Recomendado
                      </span>
                    )}
                  </div>
                  <div className="shrink-0">
                    {p.isConnected ? (
                      <span className="text-emerald-400 text-[11px] font-sans font-semibold bg-emerald-950/40 border border-emerald-500/40 px-2.5 py-1 rounded-lg flex items-center gap-1.5 shadow-[0_0_8px_rgba(16,185,129,0.2)]">
                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                        Conectado
                      </span>
                    ) : (
                      <button
                        onClick={() => onSelectPort(p.index)}
                        className="px-3 py-1 rounded-lg bg-white/[0.06] hover:bg-white/[0.12] text-zinc-300 hover:text-white border border-white/10 text-[11px] font-sans font-medium transition flex items-center gap-1 cursor-pointer"
                      >
                        Conectar
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Mapeos Hardware (MIDI Learn) */}
          <div className="pt-4 border-t border-white/[0.08]">
            <div className="flex items-center justify-between">
              <div>
                <label className="text-zinc-400 font-semibold uppercase tracking-wider text-[10.5px] font-sans">
                  Mapeos Hardware (MIDI Learn)
                </label>
                <p className="text-zinc-500 text-[11px] font-sans mt-0.5">
                  Restablece los Faders, Knobs y Pads a los mensajes estándar del Nektar LX49+.
                </p>
              </div>
              {onResetMidiMappings && (
                <button
                  type="button"
                  onClick={() => {
                    onResetMidiMappings();
                    setIsResetConfirm(true);
                    setTimeout(() => setIsResetConfirm(false), 2500);
                  }}
                  className={`px-3 py-1.5 rounded-lg border text-[11px] font-sans font-medium transition flex items-center gap-1.5 cursor-pointer shrink-0 ml-3 ${
                    isResetConfirm
                      ? "bg-emerald-500/20 text-emerald-300 border-emerald-500/50 shadow-[0_0_10px_rgba(16,185,129,0.3)]"
                      : "bg-white/[0.06] hover:bg-white/[0.12] text-zinc-300 hover:text-white border-white/10"
                  }`}
                  title="Restablecer a valores de fábrica"
                >
                  <RotateCcw size={12} className={isResetConfirm ? "text-emerald-400 animate-spin" : "text-zinc-400"} />
                  {isResetConfirm ? "¡Restablecido a Fábrica!" : "Restablecer de Fábrica"}
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-6 py-3.5 bg-black/50 border-t border-white/[0.08] flex justify-end rounded-b-2xl">
          <button
            onClick={onClose}
            className="px-5 py-1.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 text-black font-sans font-bold text-xs tracking-wider transition shadow-[0_0_12px_rgba(16,185,129,0.4)] cursor-pointer"
          >
            Listo
          </button>
        </div>
      </div>
    </div>
  );
};
