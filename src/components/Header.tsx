import React, { useState } from "react";
import { Sliders } from "lucide-react";
import { NormalizedMidiEvent } from "../types/midi";
import { MidiDinIcon, MidiStreamPopover } from "./MidiStreamPopover";

export interface AudioStatusInfo {
  deviceName?: string;
  sampleRate?: number;
  bufferSize?: number;
  isRunning?: boolean;
  device_name?: string;
  sample_rate?: number;
  buffer_size?: number;
  is_running?: boolean;
}

interface HeaderProps {
  onOpenSettings: () => void;
  midiEvents?: NormalizedMidiEvent[];
  onClearMidiEvents?: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  onOpenSettings,
  midiEvents = [],
  onClearMidiEvents,
}) => {
  const [isMidiPopoverOpen, setIsMidiPopoverOpen] = useState(false);

  return (
    <header className="h-14 bg-[#08080a]/60 backdrop-blur-md border-b border-white/[0.08] px-4 md:px-6 flex items-center justify-center z-30 shadow-[0_4px_30px_rgba(0,0,0,0.5),inset_0_1px_0_0_rgba(255,255,255,0.06)] w-full transition-all">
      <div className="studio-console flex items-center justify-between">
        {/* Brand Logo minimalista y limpio */}
        <div>
          <h1 className="text-sm font-black tracking-widest text-white flex items-center select-none">
            IMPACT<span className="text-emerald-400">SAMPLER</span>
          </h1>
        </div>

        {/* Lado Derecho: Monitor MIDI en Vivo & Botón de Ajustes */}
        <div className="flex items-center space-x-2.5">
          {/* Live MIDI Stream Popover Trigger */}
          <div className="relative">
            <button
              type="button"
              onClick={() => setIsMidiPopoverOpen((prev) => !prev)}
              className={`p-2 rounded-xl border transition flex items-center justify-center shadow-sm cursor-pointer ${
                isMidiPopoverOpen
                  ? "bg-emerald-500/20 text-emerald-300 border-emerald-500/50 shadow-[0_0_12px_rgba(16,185,129,0.3)]"
                  : "bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border-white/[0.08]"
              }`}
              title="Monitor MIDI en Vivo (Eventos en tiempo real)"
            >
              <MidiDinIcon size={16} />
            </button>
            <MidiStreamPopover
              isOpen={isMidiPopoverOpen}
              onClose={() => setIsMidiPopoverOpen(false)}
              events={midiEvents}
              onClear={onClearMidiEvents}
            />
          </div>

          {/* Settings Button */}
          <button
            type="button"
            onClick={onOpenSettings}
            className="p-2 rounded-xl bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] transition shadow-sm cursor-pointer"
            title="Configuración de Audio y MIDI"
          >
            <Sliders size={16} />
          </button>
        </div>
      </div>
    </header>
  );
};
