import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Settings } from "lucide-react";
import { FxRouting } from "../types/sampler";
import { MidiLearnTarget, MidiLearnMapping, getActionMappingLabel } from "../types/midiLearn";
import { ArpMode, ArpRate, ArpStateInfo } from "../types/arpeggiator";

interface PerformanceBayProps {
  fxRouting?: FxRouting;
  onChangeFxRouting?: (routing: FxRouting) => void;
  isMidiLearnActive?: boolean;
  selectedLearnTarget?: MidiLearnTarget | null;
  onSelectLearnTarget?: (target: MidiLearnTarget) => void;
  midiLearnMapping?: MidiLearnMapping;
}

export const PerformanceBay: React.FC<PerformanceBayProps> = ({
  fxRouting = "off",
  onChangeFxRouting,
  isMidiLearnActive = false,
  selectedLearnTarget = null,
  onSelectLearnTarget,
  midiLearnMapping,
}) => {
  // Estado del arpegiador gestionado desde Rust
  const [arpState, setArpState] = useState<ArpStateInfo>({
    enabled: false,
    hold: false,
    mode: "up",
    rate: "sixteenth",
    octaves: 1,
  });

  const [isPopoverOpen, setIsPopoverOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  // Cargar estado inicial desde Rust
  useEffect(() => {
    invoke<ArpStateInfo>("get_arp_state")
      .then((state) => setArpState(state))
      .catch((err) => console.error("Error al obtener estado del arpegiador:", err));
  }, []);

  // Escuchar conmutación disparada por hardware MIDI mapeado
  useEffect(() => {
    const handleMidiToggleArp = () => {
      setArpState((prev) => {
        const nextVal = !prev.enabled;
        invoke("set_arp_enabled", { enabled: nextVal }).catch(console.error);
        return { ...prev, enabled: nextVal };
      });
    };

    const handleMidiToggleHold = () => {
      setArpState((prev) => {
        const nextVal = !prev.hold;
        invoke("set_arp_hold", { hold: nextVal }).catch(console.error);
        return { ...prev, hold: nextVal };
      });
    };

    window.addEventListener("midi-toggle-arp", handleMidiToggleArp);
    window.addEventListener("midi-toggle-arp-hold", handleMidiToggleHold);

    return () => {
      window.removeEventListener("midi-toggle-arp", handleMidiToggleArp);
      window.removeEventListener("midi-toggle-arp-hold", handleMidiToggleHold);
    };
  }, []);

  // Cierre de popover al hacer clic fuera
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(event.target as Node) &&
        buttonRef.current &&
        !buttonRef.current.contains(event.target as Node)
      ) {
        setIsPopoverOpen(false);
      }
    };

    if (isPopoverOpen) {
      window.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isPopoverOpen]);

  // Handlers para FX Routing
  const handleToggleFxKeys = () => {
    if (isMidiLearnActive) {
      onSelectLearnTarget?.({ type: "action", id: "fxRouteKeys" });
      return;
    }
    const hasKeys = fxRouting === "keys" || fxRouting === "both";
    const hasPads = fxRouting === "pads" || fxRouting === "both";
    if (hasKeys) {
      onChangeFxRouting?.(hasPads ? "pads" : "off");
    } else {
      onChangeFxRouting?.(hasPads ? "both" : "keys");
    }
  };

  const handleToggleFxPads = () => {
    if (isMidiLearnActive) {
      onSelectLearnTarget?.({ type: "action", id: "fxRoutePads" });
      return;
    }
    const hasKeys = fxRouting === "keys" || fxRouting === "both";
    const hasPads = fxRouting === "pads" || fxRouting === "both";
    if (hasPads) {
      onChangeFxRouting?.(hasKeys ? "keys" : "off");
    } else {
      onChangeFxRouting?.(hasKeys ? "both" : "pads");
    }
  };

  // Handlers para Arpegiador
  const handleToggleArpEnabled = () => {
    if (isMidiLearnActive) {
      onSelectLearnTarget?.({ type: "action", id: "toggleArp" });
      return;
    }
    const nextVal = !arpState.enabled;
    setArpState((prev) => ({ ...prev, enabled: nextVal }));
    invoke("set_arp_enabled", { enabled: nextVal }).catch(console.error);
  };

  const handleToggleArpHold = () => {
    if (isMidiLearnActive) {
      onSelectLearnTarget?.({ type: "action", id: "toggleArpHold" });
      return;
    }
    const nextVal = !arpState.hold;
    setArpState((prev) => ({ ...prev, hold: nextVal }));
    invoke("set_arp_hold", { hold: nextVal }).catch(console.error);
  };

  const handleSelectMode = (mode: ArpMode) => {
    setArpState((prev) => ({ ...prev, mode }));
    invoke("set_arp_mode", { mode }).catch(console.error);
  };

  const handleSelectRate = (rate: ArpRate) => {
    setArpState((prev) => ({ ...prev, rate }));
    invoke("set_arp_rate", { rate }).catch(console.error);
  };

  const handleSelectOctaves = (octaves: number) => {
    setArpState((prev) => ({ ...prev, octaves }));
    invoke("set_arp_octaves", { octaves }).catch(console.error);
  };

  const getRateLabel = (rate: ArpRate): string => {
    switch (rate) {
      case "quarter":
        return "1/4";
      case "eighth":
        return "1/8";
      case "sixteenth":
        return "1/16";
      case "thirtySecond":
        return "1/32";
    }
  };

  const getModeLabel = (mode: ArpMode): string => {
    switch (mode) {
      case "up":
        return "UP";
      case "down":
        return "DOWN";
      case "upDown":
        return "U/D";
    }
  };

  const isKeysLearnSelected =
    isMidiLearnActive &&
    selectedLearnTarget?.type === "action" &&
    selectedLearnTarget.id === "fxRouteKeys";
  const keysMapped = getActionMappingLabel(midiLearnMapping, "fxRouteKeys");

  const isPadsLearnSelected =
    isMidiLearnActive &&
    selectedLearnTarget?.type === "action" &&
    selectedLearnTarget.id === "fxRoutePads";
  const padsMapped = getActionMappingLabel(midiLearnMapping, "fxRoutePads");

  const isArpLearnSelected =
    isMidiLearnActive &&
    selectedLearnTarget?.type === "action" &&
    selectedLearnTarget.id === "toggleArp";
  const arpMapped = getActionMappingLabel(midiLearnMapping, "toggleArp");

  const isHoldLearnSelected =
    isMidiLearnActive &&
    selectedLearnTarget?.type === "action" &&
    selectedLearnTarget.id === "toggleArpHold";
  const holdMapped = getActionMappingLabel(midiLearnMapping, "toggleArpHold");

  const isFxKeysActive = fxRouting === "keys" || fxRouting === "both";
  const isFxPadsActive = fxRouting === "pads" || fxRouting === "both";

  // Estilos visuales compartidos por los 4 botones de la columna (solo apariencia)
  const fxBtnBase =
    "w-full h-7 px-2 rounded-md flex items-center justify-center gap-1.5 font-mono text-[9.5px] tracking-wider transition-all active:scale-95 cursor-pointer shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_1px_3px_rgba(0,0,0,0.5)]";
  const fxBtnOff =
    "bg-[#18191e] text-zinc-400 hover:text-zinc-200 hover:bg-[#24262d] border border-white/10 hover:border-white/20 font-bold";
  const fxBtnCyan =
    "bg-cyan-950/30 text-cyan-300 border border-cyan-400/60 shadow-[0_0_6px_rgba(34,211,238,0.22),inset_0_1px_0_rgba(255,255,255,0.08)] font-black";
  const fxBtnLearnSel =
    "border-2 border-amber-400 bg-amber-950/40 text-amber-200 shadow-[0_0_15px_rgba(245,158,11,0.8)] ring-2 ring-amber-400/60 animate-pulse font-bold";
  const fxBtnLearn =
    "border border-amber-500/60 bg-amber-950/20 text-amber-300 hover:border-amber-400 font-semibold";

  // LED indicador a la izquierda del texto
  const renderLed = (on: boolean, colorClass: string, glow: string) => (
    <span
      className={`w-1 h-1 rounded-full shrink-0 transition-all ${
        on ? `${colorClass} ${glow}` : "bg-zinc-700"
      }`}
    />
  );

  return (
    <div className="w-full flex flex-col items-center justify-center gap-2 relative">
      {/* 1. Cápsula Unificada de Controles ARP [ ARP | HOLD | ⚙ ] */}
      <div className="flex items-stretch w-full h-7 rounded-md bg-[#18191e] border border-white/10 shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_1px_3px_rgba(0,0,0,0.5)] overflow-hidden">
        {/* Segmento ARP */}
        <button
          type="button"
          onClick={handleToggleArpEnabled}
          title={
            isMidiLearnActive
              ? `Arpegiador: ${arpMapped ? `Asignado (${arpMapped})` : "Sin asignar"}`
              : arpState.enabled
              ? "Desactivar Arpegiador"
              : "Activar Arpegiador"
          }
          className={`flex-1 h-full px-1.5 flex items-center justify-center gap-1.5 font-mono text-[9.5px] tracking-wider transition-all cursor-pointer border-r border-white/10 ${
            isArpLearnSelected
              ? "border-2 border-amber-400 bg-amber-950/40 text-amber-200 shadow-[0_0_15px_rgba(245,158,11,0.8)] animate-pulse font-bold"
              : isMidiLearnActive
              ? "bg-amber-950/20 text-amber-300 hover:bg-amber-950/40 font-semibold"
              : arpState.enabled
              ? "bg-emerald-950/50 text-emerald-300 shadow-[inset_0_0_8px_rgba(16,185,129,0.3)] font-black"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04] font-bold"
          }`}
        >
          {!isMidiLearnActive &&
            renderLed(arpState.enabled, "bg-emerald-400", "shadow-[0_0_5px_#34d399]")}
          ARP
        </button>

        {/* Segmento HOLD */}
        <button
          type="button"
          onClick={handleToggleArpHold}
          title={
            isMidiLearnActive
              ? `Hold / Latch: ${holdMapped ? `Asignado (${holdMapped})` : "Sin asignar"}`
              : arpState.hold
              ? "Desactivar Hold / Latch"
              : "Activar Hold / Latch"
          }
          className={`flex-1 h-full px-1.5 flex items-center justify-center gap-1.5 font-mono text-[9.5px] tracking-wider transition-all cursor-pointer border-r border-white/10 ${
            isHoldLearnSelected
              ? "border-2 border-amber-400 bg-amber-950/40 text-amber-200 shadow-[0_0_15px_rgba(245,158,11,0.8)] animate-pulse font-bold"
              : isMidiLearnActive
              ? "bg-amber-950/20 text-amber-300 hover:bg-amber-950/40 font-semibold"
              : arpState.hold
              ? "bg-amber-950/50 text-amber-300 shadow-[inset_0_0_8px_rgba(245,158,11,0.3)] font-black"
              : "text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04] font-bold"
          } ${!arpState.enabled && !isMidiLearnActive ? "opacity-40" : ""}`}
        >
          {!isMidiLearnActive &&
            renderLed(arpState.hold, "bg-amber-400", "shadow-[0_0_5px_#fbbf24]")}
          HOLD
        </button>

        {/* Segmento Ajustes (Engranaje) */}
        <button
          ref={buttonRef}
          type="button"
          onClick={() => setIsPopoverOpen(!isPopoverOpen)}
          title="Ajustes del Arpegiador"
          className={`w-7 h-full flex items-center justify-center shrink-0 transition-all cursor-pointer ${
            isPopoverOpen
              ? "bg-[#24262d] text-emerald-300 shadow-[inset_0_0_8px_rgba(52,211,153,0.35)]"
              : "text-zinc-500 hover:text-zinc-200 hover:bg-white/[0.04]"
          }`}
        >
          <Settings className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Separador sutil */}
      <div className="w-full h-px bg-white/[0.08] my-0.5" />

      {/* 2. Sección FX ROUTE (Limpia: solo título y botones KEYS / PADS) */}
      <div className="flex flex-col items-center w-full gap-1.5">
        <span className="text-[9px] font-mono uppercase tracking-wider text-zinc-400 font-bold leading-none">
          FX ROUTE
        </span>

        <div className="flex flex-col gap-1.5 w-full">
          {/* Botón KEYS */}
          <button
            type="button"
            onClick={handleToggleFxKeys}
            title={
              isMidiLearnActive
                ? `FX Route (Keys): ${keysMapped ? `Asignado (${keysMapped})` : "Sin asignar"}`
                : "FX a Teclado"
            }
            className={`${fxBtnBase} ${
              isKeysLearnSelected
                ? fxBtnLearnSel
                : isMidiLearnActive
                ? fxBtnLearn
                : isFxKeysActive
                ? fxBtnCyan
                : fxBtnOff
            }`}
          >
            {!isMidiLearnActive &&
              renderLed(isFxKeysActive, "bg-cyan-400", "shadow-[0_0_5px_#22d3ee]")}
            KEYS
          </button>

          {/* Botón PADS */}
          <button
            type="button"
            onClick={handleToggleFxPads}
            title={
              isMidiLearnActive
                ? `FX Route (Pads): ${padsMapped ? `Asignado (${padsMapped})` : "Sin asignar"}`
                : "FX a Pads"
            }
            className={`${fxBtnBase} ${
              isPadsLearnSelected
                ? fxBtnLearnSel
                : isMidiLearnActive
                ? fxBtnLearn
                : isFxPadsActive
                ? fxBtnCyan
                : fxBtnOff
            }`}
          >
            {!isMidiLearnActive &&
              renderLed(isFxPadsActive, "bg-cyan-400", "shadow-[0_0_5px_#22d3ee]")}
            PADS
          </button>
        </div>
      </div>

      {/* Popover Flotante de Ajustes Noir-Tech (anclado a la izquierda de la placa ARP) */}
      {isPopoverOpen && (
        <div
          ref={popoverRef}
          className="absolute right-full top-0 mr-2 z-50 w-56 bg-[#0c0d12] border border-white/10 rounded-xl shadow-[0_15px_35px_rgba(0,0,0,0.95)] p-2.5 font-mono text-zinc-300 animate-in fade-in zoom-in-95 duration-100 flex flex-col gap-2.5 backdrop-blur-md"
        >
          <div className="flex items-center justify-between border-b border-white/[0.08] pb-1.5">
            <span className="text-[9.5px] font-bold uppercase tracking-wider text-emerald-400 flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_#10b981]" />
              Ajustes de Arpegiador
            </span>
            <span className="text-[8px] text-zinc-500 font-semibold uppercase">Teclado</span>
          </div>

          {/* Selector de Modo (3 Modos) */}
          <div className="flex flex-col gap-1">
            <span className="text-[8.5px] font-bold text-zinc-400 uppercase tracking-wide">
              Modo de Patrón
            </span>
            <div className="grid grid-cols-3 gap-1 bg-black/40 p-0.5 rounded-md border border-white/[0.06]">
              {(["up", "down", "upDown"] as ArpMode[]).map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => handleSelectMode(m)}
                  className={`py-1 text-[8.5px] font-bold rounded transition-all cursor-pointer ${
                    arpState.mode === m
                      ? "bg-emerald-500/20 text-emerald-300 border border-emerald-400/50 shadow-[0_0_6px_rgba(16,185,129,0.3)]"
                      : "text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04]"
                  }`}
                >
                  {getModeLabel(m)}
                </button>
              ))}
            </div>
          </div>

          {/* Selector de Rate (4 Divisiones Rectas) */}
          <div className="flex flex-col gap-1">
            <span className="text-[8.5px] font-bold text-zinc-400 uppercase tracking-wide">
              División de Tempo (Rate)
            </span>
            <div className="grid grid-cols-4 gap-1 bg-black/40 p-0.5 rounded-md border border-white/[0.06]">
              {(["quarter", "eighth", "sixteenth", "thirtySecond"] as ArpRate[]).map((r) => (
                <button
                  key={r}
                  type="button"
                  onClick={() => handleSelectRate(r)}
                  className={`py-1 text-[8.5px] font-bold rounded transition-all cursor-pointer ${
                    arpState.rate === r
                      ? "bg-emerald-500/20 text-emerald-300 border border-emerald-400/50 shadow-[0_0_6px_rgba(16,185,129,0.3)]"
                      : "text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04]"
                  }`}
                >
                  {getRateLabel(r)}
                </button>
              ))}
            </div>
          </div>

          {/* Selector de Octavas (1, 2, 3) */}
          <div className="flex flex-col gap-1">
            <span className="text-[8.5px] font-bold text-zinc-400 uppercase tracking-wide">
              Rango de Octavas
            </span>
            <div className="grid grid-cols-3 gap-1 bg-black/40 p-0.5 rounded-md border border-white/[0.06]">
              {[1, 2, 3].map((oct) => (
                <button
                  key={oct}
                  type="button"
                  onClick={() => handleSelectOctaves(oct)}
                  className={`py-1 text-[8.5px] font-bold rounded transition-all cursor-pointer ${
                    arpState.octaves === oct
                      ? "bg-emerald-500/20 text-emerald-300 border border-emerald-400/50 shadow-[0_0_6px_rgba(16,185,129,0.3)]"
                      : "text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04]"
                  }`}
                >
                  {oct} {oct === 1 ? "OCT" : "OCTS"}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
