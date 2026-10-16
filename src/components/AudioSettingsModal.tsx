import React, { useEffect, useState, useRef } from "react";
import { X, Speaker, Cpu, RefreshCw, ChevronDown, Check, RotateCcw, Activity, Sliders } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AudioDeviceInfo, MidiPortInfo, NormalizedMidiEvent } from "../types/midi";
import { AudioStatusInfo } from "./Header";

interface AudioDeviceDetails {
  name: string;
  isAvailable: boolean;
  channels: number;
  sampleRates: number[];
  currentSampleRate: number;
  currentBufferSize: number;
}

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
  disabled?: boolean;
}

function CustomSelect<T extends string | number>({
  value,
  onChange,
  options,
  icon,
  placeholder = "Seleccionar...",
  disabled = false,
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
        disabled={disabled}
        onClick={() => !disabled && setIsOpen((prev) => !prev)}
        className={`w-full bg-black/60 ${
          disabled
            ? "opacity-40 cursor-not-allowed border-white/5"
            : "hover:bg-black/80 cursor-pointer " +
              (isOpen ? "border-cyan-400 shadow-[0_0_14px_rgba(6,182,212,0.25)]" : "border-white/10 hover:border-white/20")
        } rounded-xl py-2 px-3.5 flex items-center justify-between text-left transition-all shadow-[inset_0_1px_2px_rgba(0,0,0,0.8)] focus:outline-none`}
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
  latencyCompensationTicks?: number;
  onSetLatencyCompensation?: (ticks: number) => void;
  activePortName?: string;
}

export const AudioSettingsModal: React.FC<AudioSettingsModalProps> = ({
  isOpen,
  onClose,
  ports,
  onRefreshMidi,
  onSelectPort,
  onResetMidiMappings,
  latencyCompensationTicks = 0,
  onSetLatencyCompensation,
  activePortName = "",
}) => {
  const [availableHosts, setAvailableHosts] = useState<string[]>(["WASAPI", "ASIO"]);
  const [selectedHost, setSelectedHost] = useState<string>("WASAPI");
  const [audioDevices, setAudioDevices] = useState<AudioDeviceInfo[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string>("");
  const [selectedChannelPair, setSelectedChannelPair] = useState<number>(0);
  const [bufferSize, setBufferSize] = useState<number>(256);
  const [sampleRate, setSampleRate] = useState<number>(48000);
  const [isApplying, setIsApplying] = useState<boolean>(false);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [isResetConfirm, setIsResetConfirm] = useState<boolean>(false);
  const [isOpeningPanel, setIsOpeningPanel] = useState<boolean>(false);

  // Estado reactivo del dispositivo seleccionado
  const [isDeviceAvailable, setIsDeviceAvailable] = useState<boolean>(true);
  const [activeDeviceChannels, setActiveDeviceChannels] = useState<number>(2);
  const [activeDeviceSampleRates, setActiveDeviceSampleRates] = useState<number[]>([44100, 48000]);
  const [lastSelectedDeviceByHost, setLastSelectedDeviceByHost] = useState<Record<string, string>>({});

  const probeDevice = async (host: string, devName: string) => {
    if (!devName) {
      setIsDeviceAvailable(false);
      return;
    }
    try {
      const details = await invoke<AudioDeviceDetails>("get_device_details", {
        host,
        deviceName: devName,
      });
      if (details) {
        setIsDeviceAvailable(details.isAvailable);
        setActiveDeviceChannels(details.channels > 0 ? details.channels : 2);
        if (details.sampleRates && details.sampleRates.length > 0) {
          setActiveDeviceSampleRates(details.sampleRates);
        }
        if (details.isAvailable) {
          if (details.currentBufferSize) setBufferSize(details.currentBufferSize);
          if (details.currentSampleRate) setSampleRate(details.currentSampleRate);
        }
      }
    } catch (e) {
      console.warn("No se pudieron consultar detalles del dispositivo:", e);
      setIsDeviceAvailable(false);
    }
  };

  // Estados del Asistente de Calibración de Latencia (Etapa 4)
  const [isCalibrating, setIsCalibrating] = useState<boolean>(false);
  const [calibSession, setCalibSession] = useState<number>(0);
  const [calibPhase, setCalibPhase] = useState<"countdown" | "measuring" | "result">("countdown");
  const [calibCountdown, setCalibCountdown] = useState<number>(4);
  const [calibHits, setCalibHits] = useState<number[]>([]);
  const [calibOffsetResult, setCalibOffsetResult] = useState<number | null>(null);

  useEffect(() => {
    if (isOpen) {
      onRefreshMidi();
      setApplyError(null);
      // Detener y liberar el dispositivo de audio al abrir las opciones (estilo REAPER)
      invoke<AudioStatusInfo>("suspend_audio_engine").catch(console.error);

      Promise.all([
        invoke<string[]>("list_audio_hosts"),
        invoke<AudioStatusInfo>("get_audio_status"),
      ])
        .then(async ([hosts, status]) => {
          if (hosts && hosts.length > 0) {
            setAvailableHosts(hosts);
          }
          const rawHost = status?.host || status?.driver || "WASAPI";
          const activeHost = rawHost.toUpperCase().includes("ASIO") ? "ASIO" : "WASAPI";
          setSelectedHost(activeHost);

          if (status?.bufferSize || status?.buffer_size) {
            setBufferSize(status.bufferSize || status.buffer_size || 256);
          }
          if (status?.sampleRate || status?.sample_rate) {
            setSampleRate(status.sampleRate || status.sample_rate || 48000);
          }

          // Leer par de canales guardado en localStorage si existe
          try {
            const savedPrefs = JSON.parse(localStorage.getItem("impact_sampler_audio_preferences") || "{}");
            if (typeof savedPrefs.channelPair === "number") {
              setSelectedChannelPair(savedPrefs.channelPair);
            }
          } catch (_) {}

          try {
            const devs = await invoke<AudioDeviceInfo[]>("list_audio_devices", { host: activeHost });
            setAudioDevices(devs);
            const activeDeviceName = status?.deviceName || status?.device_name;
            const activeDev = devs.find(
              (d) =>
                activeDeviceName &&
                (d.name.trim().toLowerCase() === activeDeviceName.trim().toLowerCase() ||
                 d.name.toLowerCase().includes(activeDeviceName.toLowerCase()) ||
                 activeDeviceName.toLowerCase().includes(d.name.toLowerCase()))
            );
            const defaultDev = devs.find((d) => d.isDefault || (d as any).is_default);
            const resolvedDev = activeDev || defaultDev || (devs.length > 0 ? devs[0] : null);
            if (resolvedDev) {
              setSelectedDevice(resolvedDev.id);
              setLastSelectedDeviceByHost((prev) => ({ ...prev, [activeHost]: resolvedDev.id }));
              probeDevice(activeHost, resolvedDev.name);
            } else {
              setSelectedDevice("");
              setIsDeviceAvailable(false);
            }
          } catch (e) {
            console.error("Error al obtener lista de dispositivos:", e);
          }
        })
        .catch(console.error);
    }
  }, [isOpen]);

  useEffect(() => {
    const unlistenPromise = listen<AudioStatusInfo>("audio-status-changed", (event) => {
      const st = event.payload;
      if (!st) return;
      const h = st.host || st.driver;
      if (h) {
        setSelectedHost(h.toUpperCase().includes("ASIO") ? "ASIO" : "WASAPI");
      }
      const b = st.bufferSize || st.buffer_size;
      if (b) setBufferSize(b);
      const s = st.sampleRate || st.sample_rate;
      if (s) setSampleRate(s);
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  const handleHostChange = async (newHost: string) => {
    setSelectedHost(newHost);
    try {
      const devs = await invoke<AudioDeviceInfo[]>("list_audio_devices", { host: newHost });
      setAudioDevices(devs);

      // 1. Priorizar el dispositivo recordado recientemente para este host en la sesión
      const rememberedId = lastSelectedDeviceByHost[newHost];
      const rememberedDev = devs.find((d) => d.id === rememberedId);

      // 2. Si no hay en memoria de sesión, buscar en las preferencias persistidas
      let targetDev = rememberedDev;
      if (!targetDev) {
        try {
          const savedPrefs = JSON.parse(localStorage.getItem("impact_sampler_audio_preferences") || "{}");
          if (savedPrefs.host === newHost && savedPrefs.deviceName) {
            targetDev = devs.find((d) => d.name.trim().toLowerCase() === savedPrefs.deviceName.trim().toLowerCase());
          }
        } catch (_) {}
      }

      // 3. Fallback: buscar el dispositivo marcado como predeterminado (WASAPI), o el primero
      const defaultDev = devs.find((d) => d.isDefault || (d as any).is_default);
      const chosenDev = targetDev || defaultDev || (devs.length > 0 ? devs[0] : null);

      if (chosenDev) {
        setSelectedDevice(chosenDev.id);
        setLastSelectedDeviceByHost((prev) => ({ ...prev, [newHost]: chosenDev.id }));
        probeDevice(newHost, chosenDev.name);
      } else {
        setSelectedDevice("");
        setIsDeviceAvailable(false);
      }
    } catch (e) {
      console.error("Error al cambiar host de audio:", e);
    }
  };

  const handleDeviceChange = (newDeviceId: string) => {
    setSelectedDevice(newDeviceId);
    setLastSelectedDeviceByHost((prev) => ({ ...prev, [selectedHost]: newDeviceId }));
    const dev = audioDevices.find((d) => d.id === newDeviceId);
    if (dev) {
      probeDevice(selectedHost, dev.name);
    }
  };

  const handleOpenControlPanel = async () => {
    setIsOpeningPanel(true);
    try {
      const dev = audioDevices.find((d) => d.id === selectedDevice);
      const status = await invoke<AudioStatusInfo>("open_asio_control_panel", {
        deviceName: dev?.name || null,
      });
      if (status) {
        if (status.bufferSize || status.buffer_size) {
          setBufferSize(status.bufferSize || status.buffer_size || bufferSize);
        }
        if (status.sampleRate || status.sample_rate) {
          setSampleRate(status.sampleRate || status.sample_rate || sampleRate);
        }
        if (status.channels) {
          setActiveDeviceChannels(status.channels);
        }
      }
    } catch (err) {
      console.error("Error al abrir panel de control ASIO:", err);
    } finally {
      setIsOpeningPanel(false);
    }
  };

  // -------------------------------------------------------------
  // Lógica del Asistente de Calibración Asistida (8 Golpes)
  // -------------------------------------------------------------
  useEffect(() => {
    if (!isCalibrating) return;

    let audioCtx: AudioContext | null = null;
    let clickTimer: any = null;
    let nextClickTime = 0;
    let clickCount = 0;
    const bpm = 120;
    const intervalSec = 60.0 / bpm; // 500 ms por negra
    const clicks: { index: number; targetTimeMs: number }[] = [];

    try {
      const AudioCtxClass = window.AudioContext || (window as any).webkitAudioContext;
      audioCtx = new AudioCtxClass();
      if (audioCtx.state === "suspended") {
        audioCtx.resume();
      }
    } catch (e) {
      console.error("Error iniciando AudioContext para calibración:", e);
    }

    const playClickTone = (isAccent: boolean, time: number) => {
      if (!audioCtx) return;
      try {
        const osc = audioCtx.createOscillator();
        const gain = audioCtx.createGain();
        osc.type = "sine";
        osc.frequency.setValueAtTime(isAccent ? 1600 : 850, time);

        gain.gain.setValueAtTime(0.35, time);
        gain.gain.exponentialRampToValueAtTime(0.0001, time + 0.025);

        osc.connect(gain);
        gain.connect(audioCtx.destination);
        osc.start(time);
        osc.stop(time + 0.03);
      } catch (_) {}
    };

    // Planificador de pulsos (Web Audio Clock preciso para latencia 0)
    const startTime = (audioCtx ? audioCtx.currentTime : 0) + 0.1;
    nextClickTime = startTime;

    const scheduleLoop = () => {
      if (!audioCtx) return;
      while (nextClickTime < audioCtx.currentTime + 0.2) {
        if (clickCount < 12) {
          const isCountdown = clickCount < 4;
          const isAccent = clickCount === 0 || clickCount === 4;
          playClickTone(isAccent, nextClickTime);

          const clickTargetMs = performance.now() + (nextClickTime - audioCtx.currentTime) * 1000;
          clicks.push({ index: clickCount, targetTimeMs: clickTargetMs });

          if (isCountdown) {
            const countRemaining = 4 - clickCount;
            setCalibCountdown(countRemaining);
            setCalibPhase("countdown");
          } else {
            setCalibPhase("measuring");
          }

          nextClickTime += intervalSec;
          clickCount++;
        } else {
          // Ya sonaron los 4 clics de cuenta previa + 8 de medición
          break;
        }
      }

      if (clickCount < 12) {
        clickTimer = setTimeout(scheduleLoop, 25);
      } else {
        // Finalizar medición tras medio segundo del último clic
        clickTimer = setTimeout(() => {
          setCalibPhase("result");
        }, 600);
      }
    };

    scheduleLoop();

    // Listener de eventos MIDI (Pads y Teclas) en tiempo real
    const unlistenMidi = listen<NormalizedMidiEvent>("midi-event", (eventPayload) => {
      const midiEvent = eventPayload.payload;
      const eventType = (midiEvent?.event?.type || "").toLowerCase();
      if (eventType !== "noteon") return;

      const hitTime = performance.now();
      // Buscar el pulso de medición (índices 4..11) más cercano
      const measuringClicks = clicks.filter((c) => c.index >= 4);
      if (measuringClicks.length === 0) return;

      let closest = measuringClicks[0];
      let minDiff = Math.abs(hitTime - closest.targetTimeMs);

      for (let i = 1; i < measuringClicks.length; i++) {
        const diff = Math.abs(hitTime - measuringClicks[i].targetTimeMs);
        if (diff < minDiff) {
          minDiff = diff;
          closest = measuringClicks[i];
        }
      }

      // Desfase en milisegundos: positivo = golpe retrasado respecto al clic
      const deltaMs = hitTime - closest.targetTimeMs;

      // Convertir ms a ticks a 120 bpm (1 compás = 2000 ms = 384 ticks -> 1 tick = 5.2083 ms)
      const deltaTicks = Math.round(deltaMs / (500.0 / 96.0));

      setCalibHits((prev) => {
        if (prev.length >= 8) return prev;
        const next = [...prev, deltaTicks];
        if (next.length === 8) {
          // Calcular la MEDIANA de los 8 golpes (inmune a outliers)
          const sorted = [...next].sort((a, b) => a - b);
          const median = Math.round((sorted[3] + sorted[4]) / 2);
          setCalibOffsetResult(median);
        }
        return next;
      });
    });

    // Silenciar el sintetizador y sampler durante la prueba para escuchar únicamente el clic
    invoke("set_midi_mute", { muted: true }).catch(console.error);

    return () => {
      invoke("set_midi_mute", { muted: false }).catch(console.error);
      if (clickTimer) clearTimeout(clickTimer);
      if (audioCtx) {
        try {
          audioCtx.close();
        } catch (_) {}
      }
      unlistenMidi.then((fn) => fn());
    };
  }, [isCalibrating, calibSession]);

  // Si la fase pasa a result y aún no se calculó resultado con 8 golpes, calcular con los que haya
  useEffect(() => {
    if (calibPhase === "result" && calibOffsetResult === null && calibHits.length > 0) {
      const sorted = [...calibHits].sort((a, b) => a - b);
      const mid = Math.floor(sorted.length / 2);
      const median = sorted.length % 2 === 0 ? Math.round((sorted[mid - 1] + sorted[mid]) / 2) : sorted[mid];
      setCalibOffsetResult(median);
    }
  }, [calibPhase, calibOffsetResult, calibHits]);

  if (!isOpen) return null;

  const currentDev = audioDevices.find((d) => d.id === selectedDevice);

  const deviceOptions = audioDevices.map((d) => ({
    value: d.id,
    label: d.name,
    isDefault: d.isDefault ?? (d as any).is_default,
  }));

  // Opciones de Salidas Físicas (Pares Estéreo)
  const channelCount = activeDeviceChannels > 0 ? activeDeviceChannels : currentDev?.channels ?? 2;
  const numPairs = Math.max(1, Math.floor(channelCount / 2));
  const channelPairOptions = Array.from({ length: numPairs }).map((_, idx) => {
    const ch1 = idx * 2 + 1;
    const ch2 = ch1 + 1;
    let subLabel = "";
    if (idx === 0) subLabel = "Salida Principal / Main";
    else if (idx === 1) subLabel = "Auriculares / Phones";
    else subLabel = `Salida Auxiliar ${idx}`;

    return {
      value: idx,
      label: `Out ${ch1}/${ch2}`,
      subLabel,
    };
  });

  // Opciones de Buffer Size con latencia dinámica según sampleRate
  const bufferSizes = [64, 128, 256, 512, 1024];
  const bufferOptions = bufferSizes.map((size) => {
    const ms = ((size / (sampleRate || 48000)) * 1000).toFixed(1);
    let speedLabel = "";
    if (size <= 64) speedLabel = "Ultra Low Latency";
    else if (size <= 128) speedLabel = "Bajo Retardo";
    else if (size <= 256) speedLabel = "Balanceado";
    else if (size <= 512) speedLabel = "Seguro";
    else speedLabel = "Alto";

    return {
      value: size,
      label: `${size} samples`,
      subLabel: `${ms} ms · ${speedLabel}`,
    };
  });

  // Opciones de Sample Rate dinámicas según el DAC del dispositivo seleccionado
  const supportedRates =
    activeDeviceSampleRates.length > 0
      ? activeDeviceSampleRates
      : currentDev?.sampleRates && currentDev.sampleRates.length > 0
      ? currentDev.sampleRates
      : [44100, 48000];

  const sampleRateOptions = supportedRates.map((rate) => {
    let subLabel = "";
    if (rate === 44100) subLabel = "CD Audio";
    else if (rate === 48000) subLabel = "Estándar Studio";
    else if (rate === 88200) subLabel = "Hi-Res (88.2 kHz)";
    else if (rate === 96000) subLabel = "Hi-Res Studio";
    else if (rate === 192000) subLabel = "Ultra Hi-Res";
    else subLabel = `${(rate / 1000).toFixed(1)} kHz`;

    return {
      value: rate,
      label: `${rate} Hz`,
      subLabel,
    };
  });

  const handleCancel = async () => {
    try {
      await invoke("resume_audio_engine");
    } catch (_) {}
    onClose();
  };

  const handleStopCalibrating = async () => {
    setIsCalibrating(false);
    try {
      await invoke("suspend_audio_engine");
    } catch (_) {}
  };

  const handleApply = async () => {
    setIsApplying(true);
    setApplyError(null);
    try {
      const dev = audioDevices.find((d) => d.id === selectedDevice);
      const deviceName = dev?.name || null;
      const isAsio = selectedHost === "ASIO";

      // Aplicación atómica de hardware en una sola llamada IPC
      const res = await invoke<AudioStatusInfo>("set_audio_config", {
        host: selectedHost,
        deviceName,
        channelPair: selectedChannelPair,
        bufferSize: isAsio ? null : bufferSize,
        sampleRate: isAsio ? null : sampleRate,
      });

      if (res) {
        if (res.bufferSize || res.buffer_size) setBufferSize(res.bufferSize || res.buffer_size || bufferSize);
        if (res.sampleRate || res.sample_rate) setSampleRate(res.sampleRate || res.sample_rate || sampleRate);
      }

      // Guardar preferencias en localStorage para persistencia
      try {
        const prefs = {
          host: selectedHost,
          deviceName,
          channelPair: selectedChannelPair,
          bufferSize,
          sampleRate,
        };
        localStorage.setItem("impact_sampler_audio_preferences", JSON.stringify(prefs));
      } catch (e) {
        console.warn("No se pudieron guardar las preferencias de audio en localStorage:", e);
      }

      onClose();
    } catch (err: any) {
      console.error("Error al aplicar configuración de audio:", err);
      setApplyError(typeof err === "string" ? err : err?.message || "Error al aplicar configuración de audio");
    } finally {
      setIsApplying(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-md flex items-center justify-center p-4">
      <div className="bg-black/85 backdrop-blur-2xl border border-white/[0.12] w-full max-w-xl rounded-2xl shadow-[0_25px_80px_rgba(0,0,0,0.98),inset_0_1px_0_rgba(255,255,255,0.08)] overflow-visible flex flex-col animate-in fade-in zoom-in-95 duration-150">
        {/* Header */}
        <div className="px-6 py-4 bg-black/50 border-b border-white/[0.08] flex items-center justify-between rounded-t-2xl">
          <div className="flex items-center space-x-2.5">
            <Cpu className="text-emerald-400" size={18} />
            <h2 className="text-xs font-bold tracking-wider uppercase text-zinc-100 font-sans">
              Configuración de Audio y Hardware
            </h2>
          </div>
          <button
            onClick={handleCancel}
            className="text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-white/[0.06] transition cursor-pointer"
          >
            <X size={18} />
          </button>
        </div>

        {/* Body */}
        <div className="p-6 space-y-6 text-xs font-sans max-h-[75vh] overflow-y-auto">
          {applyError && (
            <div className="p-3 rounded-xl bg-red-950/60 border border-red-500/50 text-red-200 text-xs">
              {applyError}
            </div>
          )}

          {/* BLOQUE 1: SISTEMA DE AUDIO Y SALIDAS */}
          <div className="space-y-4">
            <div className="flex items-center space-x-2 border-b border-white/[0.08] pb-2">
              <Speaker className="text-cyan-400" size={15} />
              <span className="text-zinc-200 font-bold uppercase tracking-wider text-[11px] font-sans">
                Sistema de Audio y Salidas
              </span>
            </div>

            {/* Selector de Sistema de Audio (WASAPI vs ASIO) */}
            <div>
              <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10px] font-sans">
                Sistema de Audio (Audio System)
              </label>
              <div className="flex bg-black/60 p-1 rounded-xl border border-white/10 gap-1">
                {availableHosts.map((h) => {
                  const isSel = selectedHost.toUpperCase() === h.toUpperCase();
                  return (
                    <button
                      key={h}
                      type="button"
                      onClick={() => handleHostChange(h)}
                      className={`flex-1 py-1.5 px-3 rounded-lg text-xs font-sans font-semibold transition cursor-pointer ${
                        isSel
                          ? "bg-cyan-500/20 text-cyan-300 border border-cyan-400/30 shadow-[0_0_12px_rgba(6,182,212,0.25)]"
                          : "text-zinc-400 hover:text-white hover:bg-white/[0.04]"
                      }`}
                    >
                      {h}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Selector de Dispositivo / Controlador ASIO */}
            <div>
              <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10px] font-sans">
                {selectedHost === "ASIO" ? "Controlador ASIO (ASIO Driver)" : "Dispositivo de Salida (Audio Device)"}
              </label>
              <div className="flex items-center gap-2">
                <div className="flex-1 min-w-0">
                  <CustomSelect
                    value={selectedDevice}
                    onChange={handleDeviceChange}
                    options={deviceOptions}
                    icon={<Speaker size={14} />}
                    placeholder={selectedHost === "ASIO" ? "Seleccionar controlador ASIO..." : "Seleccionar dispositivo..."}
                  />
                </div>
                {selectedHost === "ASIO" && (
                  <button
                    type="button"
                    disabled={!isDeviceAvailable || isOpeningPanel}
                    onClick={handleOpenControlPanel}
                    className={`px-3.5 py-2 rounded-xl text-xs font-semibold transition flex items-center gap-1.5 shrink-0 shadow-[0_0_10px_rgba(6,182,212,0.15)] ${
                      !isDeviceAvailable
                        ? "opacity-40 cursor-not-allowed bg-white/[0.03] text-zinc-500 border border-white/5"
                        : "bg-white/[0.06] hover:bg-white/[0.12] text-cyan-300 hover:text-cyan-200 border border-cyan-400/30 cursor-pointer"
                    }`}
                    title={!isDeviceAvailable ? "Dispositivo no disponible" : "Abrir panel de control del fabricante"}
                  >
                    <Sliders size={13} className={isOpeningPanel ? "animate-spin" : ""} />
                    {isOpeningPanel ? "Panel Abierto..." : "Configuración ⚙"}
                  </button>
                )}
              </div>
              {!isDeviceAvailable && (
                <div className="mt-2 p-2.5 rounded-xl bg-red-950/40 border border-red-500/30 text-red-300 text-[11px] font-sans flex items-center gap-2 shadow-[0_0_10px_rgba(239,68,68,0.1)]">
                  <span className="w-1.5 h-1.5 rounded-full bg-red-400 animate-pulse shrink-0" />
                  <span>Sin interfaz física conectada o controlador no disponible en el sistema.</span>
                </div>
              )}
            </div>

            <div>
              <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10px] font-sans">
                Salidas Físicas (Output Channels)
              </label>
              <CustomSelect
                value={selectedChannelPair}
                onChange={(val) => setSelectedChannelPair(Number(val))}
                options={channelPairOptions}
                disabled={!isDeviceAvailable}
              />
              {currentDev && channelCount > 2 ? (
                <span className="text-[10px] text-zinc-500 font-sans mt-1 block">
                  Interfaz multicanal ({channelCount} salidas DAC detectadas). Los canales inactivos se enmudecen automáticamente.
                </span>
              ) : (
                <span className="text-[10px] text-zinc-500 font-sans mt-1 block">
                  Salida estéreo estándar (Canal Izquierdo y Derecho).
                </span>
              )}
            </div>
          </div>

          {/* BLOQUE 2: RENDIMIENTO Y BUFFER */}
          <div className="space-y-4 pt-2 border-t border-white/[0.08]">
            <div className="flex items-center space-x-2 border-b border-white/[0.08] pb-2">
              <Cpu className="text-emerald-400" size={15} />
              <span className="text-zinc-200 font-bold uppercase tracking-wider text-[11px] font-sans">
                Rendimiento y Buffer de Hardware
              </span>
            </div>

            {selectedHost === "ASIO" ? (
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10px] font-sans">
                    Tamaño de Buffer Activo
                  </label>
                  <div className={`w-full bg-black/60 border ${!isDeviceAvailable ? "border-white/5 opacity-40" : "border-white/10"} rounded-xl py-2 px-3.5 flex items-center justify-between text-zinc-200 text-xs font-mono shadow-[inset_0_1px_2px_rgba(0,0,0,0.8)]`}>
                    <span className="font-medium text-zinc-100">{isDeviceAvailable ? `${bufferSize} samples` : "---"}</span>
                    <span className="text-[10.5px] text-zinc-400">
                      {isDeviceAvailable ? `~${((bufferSize / (sampleRate || 48000)) * 1000).toFixed(1)} ms` : "---"}
                    </span>
                  </div>
                </div>

                <div>
                  <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10px] font-sans">
                    Frecuencia de Muestreo Activa
                  </label>
                  <div className={`w-full bg-black/60 border ${!isDeviceAvailable ? "border-white/5 opacity-40" : "border-white/10"} rounded-xl py-2 px-3.5 flex items-center justify-between text-zinc-200 text-xs font-mono shadow-[inset_0_1px_2px_rgba(0,0,0,0.8)]`}>
                    <span className="font-medium text-zinc-100">{isDeviceAvailable ? `${sampleRate} Hz` : "---"}</span>
                    <span className="text-[9px] uppercase font-bold text-cyan-400 px-1.5 py-0.5 rounded bg-white/[0.06] border border-cyan-400/30">
                      Hardware ASIO
                    </span>
                  </div>
                </div>
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10px] font-sans">
                    Tamaño de Buffer (Buffer Size)
                  </label>
                  <CustomSelect
                    value={bufferSize}
                    onChange={(val) => setBufferSize(Number(val))}
                    options={bufferOptions}
                    disabled={!isDeviceAvailable}
                  />
                </div>

                <div>
                  <label className="text-zinc-400 block mb-2 font-semibold tracking-wider uppercase text-[10px] font-sans">
                    Frecuencia de Muestreo (Sample Rate)
                  </label>
                  <CustomSelect
                    value={sampleRate}
                    onChange={(val) => setSampleRate(Number(val))}
                    options={sampleRateOptions}
                    disabled={!isDeviceAvailable}
                  />
                </div>
              </div>
            )}
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

          {/* Calibración Asistida de Latencia (Etapa 4) */}
          <div className="pt-4 border-t border-white/[0.08]">
            <div className="flex items-center justify-between">
              <div>
                <div className="flex items-center gap-2">
                  <label className="text-zinc-400 font-semibold uppercase tracking-wider text-[10.5px] font-sans">
                    Calibración de Latencia de Grabación
                  </label>
                  {latencyCompensationTicks !== 0 && (
                    <span className="text-[9px] font-mono uppercase font-bold tracking-wider bg-emerald-500/10 text-emerald-400 px-1.5 py-0.5 rounded border border-emerald-500/30">
                      Compensado: {latencyCompensationTicks > 0 ? `+${latencyCompensationTicks}` : latencyCompensationTicks} ticks (~{Math.round(latencyCompensationTicks * 5.2)} ms)
                    </span>
                  )}
                </div>
                <p className="text-zinc-500 text-[11px] font-sans mt-0.5">
                  Sincroniza los golpes en vivo con el clic eliminando el retardo del buffer de audio y USB-MIDI.
                </p>
              </div>

              <div className="flex items-center gap-2 shrink-0 ml-3">
                {latencyCompensationTicks !== 0 && onSetLatencyCompensation && (
                  <button
                    type="button"
                    onClick={() => {
                      onSetLatencyCompensation(0);
                      const key = selectedDevice || "default";
                      try {
                        const saved = JSON.parse(localStorage.getItem("impact_sampler_latency_by_device") || "{}");
                        delete saved[key];
                        localStorage.setItem("impact_sampler_latency_by_device", JSON.stringify(saved));
                      } catch (_) {}
                    }}
                    className="px-2.5 py-1.5 rounded-lg border border-white/10 hover:border-white/20 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-zinc-200 text-[11px] font-sans font-medium transition cursor-pointer"
                    title="Restablecer compensación a 0"
                  >
                    Restablecer
                  </button>
                )}
                <button
                  type="button"
                  onClick={async () => {
                    try {
                      await invoke("resume_audio_engine");
                    } catch (e) {
                      console.error("Error al reanudar audio para calibración:", e);
                    }
                    setIsCalibrating(true);
                    setCalibPhase("countdown");
                    setCalibCountdown(4);
                    setCalibHits([]);
                    setCalibOffsetResult(null);
                    setCalibSession((prev) => prev + 1);
                  }}
                  className="px-3.5 py-1.5 rounded-lg border border-cyan-400/40 bg-cyan-950/30 hover:bg-cyan-900/40 text-cyan-300 hover:text-cyan-200 text-[11px] font-sans font-semibold tracking-wider transition shadow-[0_0_10px_rgba(6,182,212,0.2)] flex items-center gap-1.5 cursor-pointer"
                >
                  <Activity size={13} className="text-cyan-400" />
                  {latencyCompensationTicks !== 0 ? "Recalibrar" : "Calibrar"}
                </button>
              </div>
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
        <div className="px-6 py-3.5 bg-black/50 border-t border-white/[0.08] flex items-center justify-end space-x-3 rounded-b-2xl">
          <button
            type="button"
            onClick={handleCancel}
            disabled={isApplying}
            className="px-4 py-1.5 rounded-lg border border-white/10 hover:border-white/20 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white font-sans font-medium text-xs transition cursor-pointer"
          >
            Cancelar
          </button>
          <button
            type="button"
            onClick={handleApply}
            disabled={isApplying || !isDeviceAvailable}
            className="px-5 py-1.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 disabled:opacity-40 disabled:cursor-not-allowed text-black font-sans font-bold text-xs tracking-wider transition shadow-[0_0_12px_rgba(16,185,129,0.4)] cursor-pointer flex items-center gap-2"
          >
            {isApplying ? (
              <>
                <RefreshCw size={13} className="animate-spin" />
                Aplicando...
              </>
            ) : (
              "Aplicar y Guardar"
            )}
          </button>
        </div>

        {/* Modal / Overlay Interactivo del Asistente de Calibración */}
        {isCalibrating && (
          <div className="absolute inset-0 z-50 bg-black/95 backdrop-blur-2xl rounded-2xl flex flex-col items-center justify-center p-8 animate-in fade-in duration-150">
            {calibPhase === "countdown" && (
              <div className="flex flex-col items-center text-center space-y-4">
                <div className="w-16 h-16 rounded-full bg-cyan-500/10 border border-cyan-400/30 flex items-center justify-center shadow-[0_0_30px_rgba(6,182,212,0.25)]">
                  <span className="font-mono text-3xl font-black text-cyan-300 animate-pulse">
                    {calibCountdown}
                  </span>
                </div>
                <div>
                  <h3 className="text-sm font-bold text-zinc-100 uppercase tracking-widest font-sans">
                    Escucha el Clic...
                  </h3>
                  <p className="text-zinc-400 text-xs mt-1 max-w-xs font-sans">
                    Prepárate para tocar cualquier pad o tecla exactamente sobre el pulso.
                  </p>
                </div>
              </div>
            )}

            {calibPhase === "measuring" && (
              <div className="flex flex-col items-center text-center space-y-6 w-full max-w-sm">
                <div className="space-y-1">
                  <h3 className="text-sm font-bold text-zinc-100 uppercase tracking-widest font-sans flex items-center justify-center gap-2">
                    <span className="w-2.5 h-2.5 rounded-full bg-cyan-400 animate-ping" />
                    ¡Toca con el Clic!
                  </h3>
                  <p className="text-zinc-400 text-xs font-sans">
                    Golpea cualquier pad al unísono de cada pulso
                    {activePortName && (
                      <span className="block text-zinc-500 text-[10px] font-mono mt-0.5">
                        Puerto activo: {activePortName}
                      </span>
                    )}
                  </p>
                </div>

                {/* 8 Celdas de Golpes */}
                <div className="grid grid-cols-8 gap-2 w-full">
                  {Array.from({ length: 8 }).map((_, idx) => {
                    const hit = calibHits[idx];
                    const isDone = hit !== undefined;
                    return (
                      <div
                        key={idx}
                        className={`h-12 rounded-lg border flex flex-col items-center justify-center transition-all ${
                          isDone
                            ? Math.abs(hit) <= 3
                              ? "bg-emerald-500/20 border-emerald-400/60 shadow-[0_0_12px_rgba(16,185,129,0.3)]"
                              : "bg-amber-500/20 border-amber-400/60 shadow-[0_0_12px_rgba(245,158,11,0.3)]"
                            : "bg-black/60 border-white/10"
                        }`}
                      >
                        <span className="text-[10px] font-mono font-bold text-zinc-400">
                          {idx + 1}
                        </span>
                        {isDone && (
                          <span
                            className={`text-[9px] font-mono font-bold tabular-nums ${
                              Math.abs(hit) <= 3 ? "text-emerald-300" : "text-amber-300"
                            }`}
                          >
                            {hit > 0 ? `+${hit}` : hit}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>

                <div className="text-[11px] font-mono text-zinc-400">
                  Registrados: <span className="text-cyan-400 font-bold">{calibHits.length} / 8</span>
                </div>
              </div>
            )}

            {calibPhase === "result" && (
              <div className="flex flex-col items-center text-center space-y-6 w-full max-w-sm animate-in zoom-in-95 duration-150">
                <div className="w-12 h-12 rounded-full bg-emerald-500/20 border border-emerald-400/50 flex items-center justify-center shadow-[0_0_20px_rgba(16,185,129,0.3)]">
                  <Check size={24} className="text-emerald-400" />
                </div>

                <div>
                  <h3 className="text-sm font-bold text-zinc-100 uppercase tracking-widest font-sans">
                    Calibración Completada
                  </h3>
                  <div className="mt-2 p-3 rounded-xl bg-black/60 border border-white/10 text-center">
                    <span className="text-zinc-400 text-xs block">Retardo Mediano Detectado:</span>
                    <span className="text-lg font-mono font-bold text-emerald-400 tracking-wider">
                      {calibOffsetResult !== null && calibOffsetResult > 0 ? `+${calibOffsetResult}` : calibOffsetResult ?? 0} ticks
                      <span className="text-zinc-400 text-xs font-normal ml-1">
                        (~{Math.round(((calibOffsetResult ?? 0) * 5.2))} ms)
                      </span>
                    </span>
                  </div>
                  <p className="text-zinc-400 text-[11px] font-sans mt-2">
                    Esta compensación se aplicará automáticamente a cada golpe grabado en vivo.
                  </p>
                </div>

                <div className="flex items-center gap-3 w-full">
                  <button
                    type="button"
                    onClick={() => {
                      setCalibPhase("countdown");
                      setCalibCountdown(4);
                      setCalibHits([]);
                      setCalibOffsetResult(null);
                      setCalibSession((prev) => prev + 1);
                    }}
                    className="flex-1 py-2 px-3 rounded-xl border border-white/10 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 font-sans text-xs font-medium transition cursor-pointer"
                  >
                    Repetir
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      if (calibOffsetResult !== null && onSetLatencyCompensation) {
                        onSetLatencyCompensation(calibOffsetResult);
                        const key = selectedDevice || "default";
                        try {
                          const saved = JSON.parse(localStorage.getItem("impact_sampler_latency_by_device") || "{}");
                          saved[key] = calibOffsetResult;
                          localStorage.setItem("impact_sampler_latency_by_device", JSON.stringify(saved));
                        } catch (_) {}
                      }
                      handleStopCalibrating();
                    }}
                    className="flex-1 py-2 px-3 rounded-xl bg-emerald-500 hover:bg-emerald-400 text-black font-sans text-xs font-bold tracking-wider transition shadow-[0_0_12px_rgba(16,185,129,0.4)] cursor-pointer"
                  >
                    Guardar y Aplicar
                  </button>
                </div>
              </div>
            )}

            {calibPhase !== "result" && (
              <button
                type="button"
                onClick={handleStopCalibrating}
                className="absolute top-4 right-4 text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-white/[0.06] transition cursor-pointer"
              >
                <X size={18} />
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
