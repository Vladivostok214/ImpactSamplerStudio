import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SlidersHorizontal } from "lucide-react";
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

  return (
    <div className="w-full flex flex-col items-center justify-center gap-2 relative">
      {/* Sección FX ROUTE */}
      <div className="flex flex-col items-center w-full gap-1.5">
        <span className="text-[9px] font-mono uppercase tracking-wider text-zinc-400 font-bold leading-none">
          FX ROUTE
        </span>

        <div className="grid grid-cols-2 gap-1.5 w-full">
          {/* Botón KEYS */}
          <button
            type="button"
            onClick={handleToggleFxKeys}
            title={
              isMidiLearnActive
                ? `FX Route (Keys): ${keysMapped ? `Asignado (${keysMapped})` : "Sin asignar"}`
                : "FX a Teclado"
            }
            className={`w-full py-1.5 px-2 rounded-md flex items-center justify-center font-mono text-[9.5px] tracking-wider transition-all active:scale-95 cursor-pointer shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_1px_3px_rgba(0,0,0,0.5)] ${
              isKeysLearnSelected
                ? "border-2 border-amber-400 bg-amber-950/40 text-amber-200 shadow-[0_0_15px_rgba(245,158,11,0.8)] ring-2 ring-amber-400/60 animate-pulse font-bold"
                : isMidiLearnActive
                ? "border border-amber-500/60 bg-amber-950/20 text-amber-300 hover:border-amber-400 font-semibold"
                : isFxKeysActive
                ? "bg-amber-950/40 text-amber-300 border border-amber-500/60 shadow-[0_0_8px_rgba(245,158,11,0.35),inset_0_1px_0_rgba(255,255,255,0.08)] font-black"
                : "bg-[#18191e] text-zinc-400 hover:text-zinc-200 hover:bg-[#24262d] border border-white/10 font-bold"
            }`}
          >
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
            className={`w-full py-1.5 px-2 rounded-md flex items-center justify-center font-mono text-[9.5px] tracking-wider transition-all active:scale-95 cursor-pointer shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_1px_3px_rgba(0,0,0,0.5)] ${
              isPadsLearnSelected
                ? "border-2 border-amber-400 bg-amber-950/40 text-amber-200 shadow-[0_0_15px_rgba(245,158,11,0.8)] ring-2 ring-amber-400/60 animate-pulse font-bold"
                : isMidiLearnActive
                ? "border border-amber-500/60 bg-amber-950/20 text-amber-300 hover:border-amber-400 font-semibold"
                : isFxPadsActive
                ? "bg-amber-950/40 text-amber-300 border border-amber-500/60 shadow-[0_0_8px_rgba(245,158,11,0.35),inset_0_1px_0_rgba(255,255,255,0.08)] font-black"
                : "bg-[#18191e] text-zinc-400 hover:text-zinc-200 hover:bg-[#24262d] border border-white/10 font-bold"
            }`}
          >
            PADS
          </button>
        </div>
      </div>

      {/* Separador Serigrafiado Ultra Fino */}
      <div className="w-full my-0.5 flex items-center justify-center px-1">
        <div className="w-full h-[1px] bg-white/[0.06]" />
      </div>

      {/* Sección Arpegiador (Sin rótulo redundante ARPEGGIATOR) */}
      <div className="flex flex-col items-center w-full gap-1.5">
        <div className="grid grid-cols-2 gap-1.5 w-full">
          {/* Botón ARP */}
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
            className={`w-full py-1.5 px-2 rounded-md flex items-center justify-center font-mono text-[9.5px] tracking-wider transition-all active:scale-95 cursor-pointer shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_1px_3px_rgba(0,0,0,0.5)] ${
              isArpLearnSelected
                ? "border-2 border-amber-400 bg-amber-950/40 text-amber-200 shadow-[0_0_15px_rgba(245,158,11,0.8)] ring-2 ring-amber-400/60 animate-pulse font-bold"
                : isMidiLearnActive
                ? "border border-amber-500/60 bg-amber-950/20 text-amber-300 hover:border-amber-400 font-semibold"
                : arpState.enabled
                ? "bg-emerald-950/40 text-emerald-300 border border-emerald-500/60 shadow-[0_0_8px_rgba(16,185,129,0.35),inset_0_1px_0_rgba(255,255,255,0.08)] font-black"
                : "bg-[#18191e] text-zinc-400 hover:text-zinc-200 hover:bg-[#24262d] border border-white/10 font-bold"
            }`}
          >
            ARP
          </button>

          {/* Botón HOLD */}
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
            className={`w-full py-1.5 px-2 rounded-md flex items-center justify-center font-mono text-[9.5px] tracking-wider transition-all active:scale-95 cursor-pointer shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_1px_3px_rgba(0,0,0,0.5)] ${
              isHoldLearnSelected
                ? "border-2 border-amber-400 bg-amber-950/40 text-amber-200 shadow-[0_0_15px_rgba(245,158,11,0.8)] ring-2 ring-amber-400/60 animate-pulse font-bold"
                : isMidiLearnActive
                ? "border border-amber-500/60 bg-amber-950/20 text-amber-300 hover:border-amber-400 font-semibold"
                : arpState.hold
                ? "bg-amber-950/40 text-amber-300 border border-amber-500/60 shadow-[0_0_8px_rgba(245,158,11,0.35),inset_0_1px_0_rgba(255,255,255,0.08)] font-black"
                : "bg-[#18191e] text-zinc-400 hover:text-zinc-200 hover:bg-[#24262d] border border-white/10 font-bold"
            }`}
          >
            HOLD
          </button>
        </div>

        {/* Botón SETTINGS Enrasado */}
        <button
          ref={buttonRef}
          type="button"
          onClick={() => setIsPopoverOpen(!isPopoverOpen)}
          title="Ajustes del Arpegiador"
          className="w-full py-1.5 px-2 rounded-md bg-[#18191e] hover:bg-[#24262d] border border-white/10 hover:border-white/20 text-zinc-400 hover:text-zinc-200 transition-all flex items-center justify-center gap-1.5 active:scale-95 cursor-pointer shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_1px_3px_rgba(0,0,0,0.5)]"
        >
          <SlidersHorizontal className="w-2.5 h-2.5" />
          <span className="text-[9px] font-mono font-bold tracking-wider uppercase">
            SETTINGS
          </span>
        </button>
      </div>

      {/* Popover Flotante de Ajustes Noir-Tech */}
      {isPopoverOpen && (
        <div
          ref={popoverRef}
          className="absolute right-0 top-full mt-1.5 z-50 w-56 bg-[#0c0d12] border border-white/10 rounded-xl shadow-[0_15px_35px_rgba(0,0,0,0.95)] p-2.5 font-mono text-zinc-300 animate-in fade-in zoom-in-95 duration-100 flex flex-col gap-2.5 backdrop-blur-md"
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
