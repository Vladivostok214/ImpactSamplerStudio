import React from "react";
import { StepResolution } from "../types/sampler";

interface GiantMetronomeOverlayProps {
  isPlaying: boolean;
  isVisible: boolean;
  beat: number; // 1..8, 1..4 o 1..2 (pulso de negra)
  maxBeats: number;
  resolution: StepResolution;
}

export const GiantMetronomeOverlay: React.FC<GiantMetronomeOverlayProps> = ({
  isPlaying,
  isVisible,
  beat,
  maxBeats,
  resolution,
}) => {
  if (!isPlaying || !isVisible) return null;

  const isDownbeat = beat === 1;

  const resolutionLabel =
    resolution === "thirtySecond"
      ? "1/32"
      : resolution === "sixteenth"
      ? "1/16"
      : "1/8";

  return (
    <div
      aria-hidden="true"
      className="fixed inset-0 z-20 pointer-events-none select-none flex flex-col items-center justify-center overflow-hidden transition-opacity duration-200"
    >
      {/* Glow ambiental central sincronizado */}
      <div
        key={`glow-${beat}`}
        className={`absolute w-[45vw] h-[45vw] rounded-full blur-[140px] transition-all duration-150 ${
          isDownbeat
            ? "bg-emerald-500/10 scale-125"
            : "bg-white/[0.04] scale-100"
        }`}
      />

      {/* Número Gigante Ultra-Minimalista con Máxima Opacidad (marca el pulso de negra) */}
      <div
        key={`num-${beat}`}
        className={`font-mono font-black text-[38vw] leading-none tracking-tighter select-none animate-pulse-short drop-shadow-[0_10px_60px_rgba(0,0,0,0.9)] drop-shadow-[0_0_80px_rgba(255,255,255,0.3)] ${
          isDownbeat
            ? "text-emerald-400 scale-105"
            : "text-white"
        }`}
        style={{
          transform: "translateY(-2%)",
        }}
      >
        {beat}
      </div>

      {/* Sub-indicador de Compás y Resolución */}
      <div className="absolute bottom-16 flex items-center gap-2 px-5 py-2 rounded-full bg-black/70 backdrop-blur-md border border-white/20 text-xs font-mono tracking-widest text-white uppercase shadow-2xl">
        <span className="inline-block w-2.5 h-2.5 rounded-full bg-emerald-400 animate-ping mr-1" />
        PULSO {beat} / {maxBeats} &bull; COMPÁS {resolution === "sixteenth" ? "1" : (beat <= 4 ? "1" : "2")} (TIEMPO {((beat - 1) % 4) + 1}/4) &bull; {resolutionLabel}
      </div>
    </div>
  );
};
