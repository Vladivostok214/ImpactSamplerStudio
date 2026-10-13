import React, { useState, useEffect } from "react";
import { X, Check, ArrowRight } from "lucide-react";
import { SoundFontInfo } from "../types/soundfont";
import pianoImg from "../assets/instruments/piano.png";
import rhodesImg from "../assets/instruments/rhodes.png";
import melotronImg from "../assets/instruments/melotron.png";
import rx7Img from "../assets/instruments/yamaharx7.png";

interface InstrumentSelectModalProps {
  isOpen: boolean;
  isInitialSplash?: boolean;
  availableSoundfonts: SoundFontInfo[];
  currentSoundfont: SoundFontInfo | null;
  onSelectSoundfont: (filename: string) => Promise<void>;
  onClose?: () => void;
}

export const InstrumentSelectModal: React.FC<InstrumentSelectModalProps> = ({
  isOpen,
  isInitialSplash = false,
  availableSoundfonts,
  currentSoundfont,
  onSelectSoundfont,
  onClose,
}) => {
  const [selectedFilename, setSelectedFilename] = useState<string | null>(null);
  const [loadingProgress, setLoadingProgress] = useState<number>(0);
  const [isLoading, setIsLoading] = useState<boolean>(false);

  useEffect(() => {
    if (currentSoundfont) {
      setSelectedFilename(currentSoundfont.filename);
    }
  }, [currentSoundfont]);

  if (!isOpen) return null;

  const handleSelect = async (sf: SoundFontInfo) => {
    if (isLoading) return;

    setSelectedFilename(sf.filename);
    setIsLoading(true);
    setLoadingProgress(20);

    const t1 = setTimeout(() => setLoadingProgress(65), 140);
    const t2 = setTimeout(() => setLoadingProgress(90), 260);

    try {
      await onSelectSoundfont(sf.filename);
      clearTimeout(t1);
      clearTimeout(t2);
      setLoadingProgress(100);

      setTimeout(() => {
        setIsLoading(false);
        if (onClose) onClose();
      }, 300);
    } catch (err) {
      clearTimeout(t1);
      clearTimeout(t2);
      setIsLoading(false);
      console.error(err);
    }
  };

  const getInstrumentConfig = (sf: SoundFontInfo) => {
    const fn = sf.filename.toLowerCase();
    if (fn.includes("piano") && !fn.includes("rhodes")) {
      return {
        title: "Grand Piano",
        subtitle: "Steinway Samples",
        accentBorder: "border-amber-400/90 shadow-[0_0_30px_rgba(245,158,11,0.3)]",
        activeLed: "bg-amber-400 shadow-[0_0_8px_#f59e0b]",
        imgSrc: pianoImg,
        glowColor: "rgba(245, 158, 11, 0.45)",
      };
    } else if (fn.includes("rhodes")) {
      return {
        title: "Rhodes Mark I",
        subtitle: "Vintage Electro-Mechanic",
        accentBorder: "border-cyan-400/90 shadow-[0_0_30px_rgba(6,182,212,0.3)]",
        activeLed: "bg-cyan-400 shadow-[0_0_8px_#22d3ee]",
        imgSrc: rhodesImg,
        glowColor: "rgba(6, 182, 212, 0.45)",
      };
    } else if (fn.includes("rx7")) {
      return {
        title: "Yamaha RX7",
        subtitle: "1987 PCM Drum Machine",
        accentBorder: "border-purple-400/90 shadow-[0_0_30px_rgba(168,85,247,0.3)]",
        activeLed: "bg-purple-400 shadow-[0_0_8px_#c084fc]",
        imgSrc: rx7Img,
        glowColor: "rgba(168, 85, 247, 0.45)",
      };
    } else {
      return {
        title: "Mellotron Tape",
        subtitle: "Classic Analog Tape",
        accentBorder: "border-rose-400/90 shadow-[0_0_30px_rgba(244,63,94,0.3)]",
        activeLed: "bg-rose-400 shadow-[0_0_8px_#f43f5e]",
        imgSrc: melotronImg,
        glowColor: "rgba(244, 63, 94, 0.45)",
      };
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/85 backdrop-blur-md flex items-center justify-center p-4 sm:p-6 select-none animate-in fade-in duration-500 ease-out">
      {/* Contenedor Modal de Vidrio Negro Translúcido Puro */}
      <div className="relative w-full max-w-5xl xl:max-w-6xl bg-black/80 backdrop-blur-2xl border border-white/[0.12] rounded-2xl shadow-[0_25px_80px_rgba(0,0,0,0.98),inset_0_1px_0_rgba(255,255,255,0.08)] overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-500 ease-out">
        {/* Encabezado Sobrio en Negro Translúcido */}
        <div className="px-6 sm:px-8 py-4 sm:py-5 flex items-center justify-between border-b border-white/[0.08] bg-black/50">
          <div className="flex items-center gap-3">
            <span className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_8px_#10b981]" />
            <h2 className="text-xs sm:text-sm font-mono font-bold tracking-widest text-zinc-100 uppercase">
              SELECCIONAR INSTRUMENTO DE ESTUDIO
            </h2>
          </div>

          {!isInitialSplash && onClose && !isLoading && (
            <button
              type="button"
              onClick={onClose}
              className="text-zinc-400 hover:text-white transition p-1.5 rounded-lg hover:bg-white/[0.06] cursor-pointer"
              title="Cerrar"
            >
              <X size={18} />
            </button>
          )}
        </div>

        {/* 4 Tarjetas de Instrumentos */}
        <div className="p-5 sm:p-7 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 sm:gap-5">
          {availableSoundfonts.map((sf) => {
            const config = getInstrumentConfig(sf);
            const isCurrent = currentSoundfont?.filename === sf.filename;
            const isSelected = selectedFilename === sf.filename;

            return (
              <div
                key={sf.filename}
                onClick={() => !isLoading && handleSelect(sf)}
                className={`group relative h-72 sm:h-80 rounded-xl border overflow-hidden transition-all duration-300 flex flex-col justify-between p-4 sm:p-5 cursor-pointer ${
                  isSelected
                    ? `${config.accentBorder} bg-black/90 scale-[1.01]`
                    : "border-white/[0.08] bg-black/70 hover:border-white/35 hover:bg-black/80 hover:scale-[1.005]"
                }`}
              >
                {/* 1. Resplandor reactivo de fondo (Underglow DETRÁS de la imagen) */}
                <div
                  className={`absolute inset-0 pointer-events-none transition-opacity duration-500 z-0 ${
                    isSelected ? "opacity-75" : "opacity-0 group-hover:opacity-40"
                  }`}
                  style={{
                    background: `radial-gradient(circle at 50% 36%, ${config.glowColor} 0%, transparent 68%)`,
                  }}
                />

                {/* 2. Imagen del Instrumento: 100% OPACIDAD, posicionada en la parte superior con mínimo margen */}
                <div className="absolute top-2 inset-x-0 h-44 sm:h-48 flex items-center justify-center p-2 pointer-events-none z-10">
                  <img
                    src={config.imgSrc}
                    alt={config.title}
                    loading="eager"
                    decoding="async"
                    className="w-full h-full object-contain filter drop-shadow-[0_12px_24px_rgba(0,0,0,0.95)] group-hover:scale-106 transition-transform duration-300"
                  />
                </div>

                {/* 3. Gradiente oscuro localizado EXCLUSIVAMENTE en la base para dar contraste al texto sin tapar el instrumento */}
                <div
                  className="absolute bottom-0 inset-x-0 h-32 bg-gradient-to-t from-black via-black/90 to-transparent pointer-events-none z-20"
                />

                {/* 4. Encabezado de la tarjeta: Badge flotante cuando el instrumento está activo en memoria */}
                <div className="relative z-30 flex items-center justify-end w-full">
                  {isCurrent && (
                    <span className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-black/80 border border-emerald-500/50 backdrop-blur-md text-[9.5px] font-mono text-emerald-300 font-bold tracking-wider shadow-[0_0_12px_rgba(16,185,129,0.35)]">
                      <span className={`w-1.5 h-1.5 rounded-full ${config.activeLed}`} />
                      ACTIVO
                    </span>
                  )}
                </div>

                {/* 5. Pie de la tarjeta: Título, Subtítulo y Acción Cargar */}
                <div className="relative z-30 flex flex-col justify-end">
                  <h3 className="text-base sm:text-lg font-extrabold text-white tracking-tight drop-shadow-[0_2px_4px_rgba(0,0,0,0.95)] truncate">
                    {config.title}
                  </h3>
                  <p className="text-[11px] font-mono text-zinc-300/90 drop-shadow-[0_1px_2px_rgba(0,0,0,0.95)] truncate mt-0.5 font-medium">
                    {config.subtitle}
                  </p>

                  <div className="mt-3 pt-2.5 border-t border-white/[0.12] flex items-center justify-between text-[11px] font-mono">
                    <span
                      className={
                        isCurrent
                          ? "text-emerald-400 flex items-center gap-1.5 font-bold drop-shadow-[0_0_8px_rgba(16,185,129,0.4)]"
                          : isSelected
                          ? "text-zinc-100 font-bold"
                          : "text-zinc-300 group-hover:text-white font-medium"
                      }
                    >
                      {isCurrent ? (
                        <>
                          <Check size={13} className="text-emerald-400 shrink-0" />
                          En memoria
                        </>
                      ) : (
                        "Cargar"
                      )}
                    </span>
                    {!isCurrent && (
                      <ArrowRight size={13} className="text-zinc-400 group-hover:text-white group-hover:translate-x-1 transition-all" />
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>

        {/* Barra de Progreso de Memoria */}
        <div className="px-6 sm:px-8 pb-5 pt-1 flex flex-col gap-2 bg-black/50 border-t border-white/[0.08]">
          <div className="flex items-center justify-between text-[11px] font-mono">
            <span className={isLoading ? "text-amber-400 animate-pulse font-medium" : "text-zinc-400"}>
              {isLoading ? "Cargando muestras de audio..." : "Motor listo"}
            </span>
            {isLoading && (
              <span className="font-bold text-zinc-200">{loadingProgress}%</span>
            )}
          </div>

          <div className="w-full h-1 bg-white/[0.08] rounded-full overflow-hidden border border-white/[0.06]">
            <div
              className="h-full bg-gradient-to-r from-emerald-500 via-cyan-400 to-amber-400 rounded-full transition-all duration-200 shadow-[0_0_8px_rgba(34,211,238,0.7)]"
              style={{
                width: isLoading ? `${loadingProgress}%` : currentSoundfont ? "100%" : "0%",
              }}
            />
          </div>
        </div>
      </div>
    </div>
  );
};
