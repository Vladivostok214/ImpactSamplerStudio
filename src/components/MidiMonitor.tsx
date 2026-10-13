import React from "react";
import { NormalizedMidiEvent } from "../types/midi";
import { Terminal, Trash2 } from "lucide-react";

interface MidiMonitorProps {
  events: NormalizedMidiEvent[];
  onClear?: () => void;
}

export const MidiMonitor: React.FC<MidiMonitorProps> = ({ events, onClear }) => {
  return (
    <div className="bg-studio-900 border border-studio-800 rounded-xl p-4 flex flex-col h-full shadow-lg">
      <div className="flex items-center justify-between pb-3 border-b border-studio-800 mb-3">
        <div className="flex items-center space-x-2">
          <Terminal size={15} className="text-studio-cyan" />
          <h2 className="text-xs font-mono font-bold tracking-wider text-slate-200 uppercase">
            Live MIDI Stream Monitor
          </h2>
          <span className="text-[10px] bg-studio-800 text-slate-400 px-2 py-0.5 rounded-full font-mono">
            {events.length} msgs
          </span>
        </div>
        {onClear && (
          <button
            onClick={onClear}
            className="text-slate-400 hover:text-slate-200 text-xs flex items-center gap-1 transition"
          >
            <Trash2 size={12} /> Limpiar
          </button>
        )}
      </div>

      <div className="flex-1 overflow-y-auto space-y-1.5 font-mono text-[11px] pr-1">
        {events.length === 0 ? (
          <div className="h-24 flex items-center justify-center text-zinc-600 text-[10px] uppercase font-mono tracking-wider select-none">
            Sin eventos
          </div>
        ) : (
          events.map((ev, i) => {
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
                className="flex items-center justify-between px-2.5 py-1 rounded bg-studio-950/70 border border-studio-800/60 hover:border-studio-700 transition"
              >
                <div className="flex items-center space-x-2">
                  <span className="text-[9.5px] px-1 py-0.5 rounded bg-studio-800 text-slate-300 uppercase font-semibold tracking-wider">
                    {badge}
                  </span>
                  <span className="text-slate-400 font-medium tabular-nums">Ch {channel}</span>
                  <span className={color}>{details}</span>
                </div>
                <span className="text-[9.5px] text-slate-400 truncate max-w-[120px] font-medium">
                  {portName}
                </span>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
