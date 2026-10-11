import React, { useState, useEffect } from "react";
import { X, Check } from "lucide-react";
import { SoundFontInfo } from "../types/soundfont";

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

  const getCleanInfo = (sf: SoundFontInfo) => {
    const fn = sf.filename.toLowerCase();
    if (fn.includes("piano") && !fn.includes("rhodes")) {
      return {
        title: "Grand Piano",
        type: "Acoustic Piano",
        details: "Steinway & Sons Samples",
        accent: "border-amber-400/80 shadow-[0_0_20px_rgba(251,191,36,0.15)]",
        badge: "text-amber-400",
      };
    } else if (fn.includes("rhodes")) {
      return {
        title: "Rhodes Mark I",
        type: "Electric Piano",
        details: "70s Vintage Electro-Mechanic",
        accent: "border-cyan-400/80 shadow-[0_0_20px_rgba(0,229,255,0.15)]",
        badge: "text-cyan-400",
      };
    } else {
      return {
        title: "Mellotron Tape",
        type: "Tape Strings & Winds",
        details: "12 Vintage Presets (G2 - F5)",
        accent: "border-red-400/80 shadow-[0_0_20px_rgba(239,68,68,0.15)]",
        badge: "text-red-400",
      };
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/85 backdrop-blur-md flex items-center justify-center p-6 select-none animate-in fade-in duration-200">
      <div className="relative w-full max-w-3xl bg-zinc-950/85 backdrop-blur-2xl border border-white/[0.08] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
        {/* Encabezado Sobrio */}
        <div className="px-8 pt-7 pb-4 flex items-center justify-between border-b border-white/[0.08]">
          <div>
            <h2 className="text-sm font-mono font-bold tracking-widest text-zinc-100 uppercase">
              SELECCIÓN DE INSTRUMENTO
            </h2>
          </div>

          {!isInitialSplash && onClose && !isLoading && (
            <button
              onClick={onClose}
              className="text-zinc-400 hover:text-white transition p-1"
              title="Cerrar"
            >
              <X size={18} />
            </button>
          )}
        </div>

        {/* 3 Tarjetas Minimalistas */}
        <div className="p-8 grid grid-cols-1 md:grid-cols-3 gap-4">
          {availableSoundfonts.map((sf) => {
            const clean = getCleanInfo(sf);
            const isCurrent = currentSoundfont?.filename === sf.filename;
            const isSelected = selectedFilename === sf.filename;

            return (
              <div
                key={sf.filename}
                onClick={() => !isLoading && handleSelect(sf)}
                className={`relative rounded-xl p-5 border transition-all duration-150 flex flex-col justify-between cursor-pointer backdrop-blur-md ${
                  isSelected
                    ? `${clean.accent} bg-white/[0.08]`
                    : "border-white/[0.08] bg-white/[0.02] hover:border-white/20 hover:bg-white/[0.05]"
                }`}
              >
                <div>
                  <div className="flex items-center justify-between mb-3">
                    <span className={`text-[10px] font-mono uppercase tracking-wider font-semibold ${clean.badge}`}>
                      {clean.type}
                    </span>
                    {isCurrent && (
                      <span className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.8)]" />
                    )}
                  </div>

                  <h3 className="text-base font-bold text-slate-100 mb-1 tracking-tight">
                    {clean.title}
                  </h3>
                  <p className="text-[11px] font-mono text-slate-400">
                    {clean.details}
                  </p>
                </div>

                <div className="mt-6 pt-3 border-t border-white/[0.08] flex items-center justify-between text-[11px] font-mono">
                  <span className={isCurrent ? "text-emerald-400 flex items-center gap-1 font-semibold" : "text-zinc-400"}>
                    {isCurrent ? (
                      <>
                        <Check size={12} />
                        Cargado
                      </>
                    ) : (
                      "Seleccionar"
                    )}
                  </span>
                </div>
              </div>
            );
          })}
        </div>

        {/* Barra de Progreso Minimalista */}
        <div className="px-8 pb-7 pt-2 flex flex-col gap-2">
          {isLoading && (
            <div className="flex items-center justify-between text-[11px] font-mono text-slate-300">
              <span className="animate-pulse">Cargando muestras en memoria RAM...</span>
              <span className="font-bold text-studio-cyan">{loadingProgress}%</span>
            </div>
          )}

          <div className="w-full h-1 bg-studio-950 rounded-full overflow-hidden border border-studio-800/60">
            <div
              className="h-full bg-studio-cyan rounded-full transition-all duration-200 shadow-[0_0_8px_rgba(0,229,255,0.7)]"
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
