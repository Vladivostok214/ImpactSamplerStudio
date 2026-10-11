import React, { useRef, useEffect } from "react";
import { NormalizedMidiEvent } from "../types/midi";
import { Zap, Trash2, X } from "lucide-react";

export const MidiDinIcon: React.FC<{ size?: number; className?: string }> = ({
  size = 16,
  className = "",
}) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.75"
    strokeLinecap="round"
    strokeLinejoin="round"
    className={className}
  >
    {/* Conector circular exterior DIN-5 */}
    <circle cx="12" cy="12" r="9.5" />
    {/* Muesca de orientación superior */}
    <path d="M12 2.5 v2.5" />
    {/* 5 pines en arco de 180° característicos del conector MIDI DIN */}
    <circle cx="7" cy="13.5" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="8.5" cy="9.2" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="12" cy="7.5" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="15.5" cy="9.2" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="17" cy="13.5" r="1.1" fill="currentColor" stroke="none" />
  </svg>
);

interface MidiStreamPopoverProps {
  isOpen: boolean;
  onClose: () => void;
  events: NormalizedMidiEvent[];
  onClear?: () => void;
}

export const MidiStreamPopover: React.FC<MidiStreamPopoverProps> = ({
  isOpen,
  onClose,
  events,
  onClear,
}) => {
  const popoverRef = useRef<HTMLDivElement>(null);

  // Cerrar al hacer clic fuera
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(event.target as Node)) {
        onClose();
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  // Tomar exactamente los últimos 10 eventos sin necesidad de scroll
  const recentEvents = events.slice(0, 10);

  return (
    <div
      ref={popoverRef}
      className="absolute right-0 top-full mt-2 w-96 z-50 bg-studio-950/92 backdrop-blur-xl border border-studio-700/80 rounded-xl shadow-[0_10px_40px_rgba(0,0,0,0.8)] p-3 text-slate-100 font-mono animate-in fade-in zoom-in-95 duration-100"
    >
      {/* Cabecera del Popover */}
      <div className="flex items-center justify-between pb-2 border-b border-studio-800/80 mb-2">
        <div className="flex items-center space-x-2">
          <MidiDinIcon size={15} className="text-studio-cyan" />
          <span className="text-[11px] font-bold tracking-wider text-slate-200 uppercase">
            LIVE MIDI STREAM
          </span>
          <span className="text-[9px] bg-studio-800 text-slate-400 px-1.5 py-0.2 rounded font-mono">
            {events.length} msgs
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          {onClear && (
            <button
              type="button"
              onClick={onClear}
              className="text-slate-400 hover:text-slate-200 text-[10px] flex items-center gap-1 transition px-1.5 py-0.5 rounded hover:bg-studio-800"
              title="Limpiar historial"
            >
              <Trash2 size={11} /> Limpiar
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="text-slate-400 hover:text-slate-200 p-0.5 rounded hover:bg-studio-800 transition"
            title="Cerrar"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Lista de últimos 10 eventos (sin scroll) */}
      <div className="space-y-1 text-[10.5px]">
        {recentEvents.length === 0 ? (
          <div className="h-32 flex flex-col items-center justify-center text-slate-500 text-xs gap-1.5">
            <Zap size={16} className="animate-pulse text-amber-500" />
            <span>Esperando actividad del controlador...</span>
          </div>
        ) : (
          recentEvents.map((ev, i) => {
            const { event, portName, channel } = ev;
            let badge = "";
            let details = "";
            let color = "text-slate-300";

            const type = (event.type || "").toLowerCase();
            const noteName = (event as any).noteName || (event as any).note_name || "";

            if (type === "noteon") {
              const noteEv = event as any;
              badge = "NOTE ON";
              details = `Nota: ${noteEv.note} (${noteName}) | Vel: ${noteEv.velocity}`;
              if (noteEv.padIndex) details += ` [PAD ${noteEv.padIndex}]`;
              color = "text-emerald-400 font-semibold";
            } else if (type === "noteoff") {
              const noteEv = event as any;
              badge = "NOTE OFF";
              details = `Nota: ${noteEv.note} (${noteName})`;
              color = "text-slate-400";
            } else if (type === "controlchange") {
              const ccEv = event as any;
              badge = "CC";
              details = `CC ${ccEv.controller} = ${ccEv.value}`;
              if (ccEv.label) details += ` [${ccEv.label}]`;
              color = "text-studio-cyan font-semibold";
            } else if (type === "pitchbend") {
              const bendEv = event as any;
              badge = "BEND";
              details = `Valor: ${bendEv.normalized}`;
              color = "text-amber-400 font-semibold";
            }

            return (
              <div
                key={i}
                className="flex items-center justify-between px-2 py-0.5 rounded bg-studio-900/60 border border-studio-800/40"
              >
                <div className="flex items-center space-x-1.5 truncate">
                  <span className="text-[8px] px-1 py-0.2 rounded bg-studio-800 text-slate-400 uppercase font-semibold">
                    {badge}
                  </span>
                  <span className="text-[9px] text-slate-500">Ch{channel}</span>
                  <span className={`truncate text-[10px] ${color}`}>{details}</span>
                </div>
                <span className="text-[8px] text-slate-500 truncate max-w-[80px] text-right ml-1">
                  {portName.replace(/Impact\s*/i, "")}
                </span>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
