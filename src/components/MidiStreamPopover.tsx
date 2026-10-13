import React, { useRef, useEffect } from "react";
import { NormalizedMidiEvent } from "../types/midi";
import { Trash2, X } from "lucide-react";

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
  triggerRef?: React.RefObject<HTMLElement | null>;
  events: NormalizedMidiEvent[];
  onClear?: () => void;
}

export const MidiStreamPopover: React.FC<MidiStreamPopoverProps> = ({
  isOpen,
  onClose,
  triggerRef,
  events,
  onClear,
}) => {
  const popoverRef = useRef<HTMLDivElement>(null);

  // Cerrar al hacer clic fuera (ignorando el botón disparador para permitir toggle)
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (triggerRef?.current && triggerRef.current.contains(target)) {
        return;
      }
      if (popoverRef.current && !popoverRef.current.contains(target)) {
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
      className="absolute right-0 top-full mt-2 w-96 z-50 bg-black/90 backdrop-blur-2xl border border-white/[0.12] rounded-2xl shadow-[0_25px_60px_rgba(0,0,0,0.98),inset_0_1px_0_rgba(255,255,255,0.08)] p-3 text-zinc-100 font-mono animate-in fade-in zoom-in-95 duration-100"
    >
      {/* Cabecera del Popover */}
      <div className="flex items-center justify-between pb-2 border-b border-white/[0.08] mb-2">
        <div className="flex items-center space-x-2">
          <MidiDinIcon size={15} className="text-cyan-400" />
          <span className="text-[11px] font-bold tracking-wider text-zinc-200 uppercase">
            LIVE MIDI STREAM
          </span>
          <span className="text-[9.5px] bg-white/[0.06] text-zinc-400 px-1.5 py-0.5 rounded font-mono font-medium tabular-nums">
            {events.length} msgs
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          {onClear && (
            <button
              type="button"
              onClick={onClear}
              className="text-zinc-400 hover:text-white text-[10px] flex items-center gap-1 transition px-1.5 py-0.5 rounded hover:bg-white/[0.08]"
              title="Limpiar historial"
            >
              <Trash2 size={11} /> Limpiar
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="text-zinc-400 hover:text-white p-0.5 rounded hover:bg-white/[0.08] transition"
            title="Cerrar"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Lista de últimos 10 eventos (sin scroll) */}
      <div className="space-y-1 text-[10.5px]">
        {recentEvents.length === 0 ? (
          <div className="py-2.5 text-center text-zinc-600 text-[10px] uppercase font-mono tracking-wider select-none">
            Sin eventos
          </div>
        ) : (
          recentEvents.map((ev, i) => {
            const { event, portName, channel } = ev;
            let badge = "";
            let details = "";
            let color = "text-zinc-300";

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
              color = "text-zinc-400";
            } else if (type === "controlchange") {
              const ccEv = event as any;
              badge = "CC";
              details = `CC ${ccEv.controller} = ${ccEv.value}`;
              if (ccEv.label) details += ` [${ccEv.label}]`;
              color = "text-cyan-400 font-semibold";
            } else if (type === "pitchbend") {
              const bendEv = event as any;
              badge = "BEND";
              details = `Valor: ${bendEv.normalized}`;
              color = "text-amber-400 font-semibold";
            }

            return (
              <div
                key={i}
                className="flex items-center justify-between px-2.5 py-1 rounded-lg bg-black/50 hover:bg-white/[0.04] border border-white/[0.06] transition-colors"
              >
                <div className="flex items-center space-x-1.5 truncate">
                  <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-white/[0.08] text-zinc-200 border border-white/10 uppercase font-semibold tracking-wider">
                    {badge}
                  </span>
                  <span className="text-[10px] text-zinc-400 font-medium tabular-nums">Ch{channel}</span>
                  <span className={`truncate text-[10.5px] ${color}`}>{details}</span>
                </div>
                <span className="text-[9.5px] text-zinc-500 truncate max-w-[80px] text-right ml-1 font-medium">
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
