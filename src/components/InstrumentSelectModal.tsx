import React, { useState, useEffect, useRef } from "react";
import { X, Check, ArrowRight, Plus, Trash2, Music, UploadCloud, ChevronLeft, ChevronRight } from "lucide-react";
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
  onImportSoundfont?: () => Promise<void>;
  onDeleteSoundfont?: (filename: string) => Promise<void>;
  onClose?: () => void;
}

export const InstrumentSelectModal: React.FC<InstrumentSelectModalProps> = ({
  isOpen,
  isInitialSplash = false,
  availableSoundfonts,
  currentSoundfont,
  onSelectSoundfont,
  onImportSoundfont,
  onDeleteSoundfont,
  onClose,
}) => {
  const [selectedFilename, setSelectedFilename] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isImporting, setIsImporting] = useState<boolean>(false);
  const [deletingFilename, setDeletingFilename] = useState<string | null>(null);

  useEffect(() => {
    if (currentSoundfont) {
      setSelectedFilename(currentSoundfont.filename);
    }
  }, [currentSoundfont]);

  const isMountedRef = useRef(true);
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const handleSelect = async (sf: SoundFontInfo) => {
    if (isLoading || isImporting || deletingFilename) return;

    setSelectedFilename(sf.filename);
    setIsLoading(true);

    try {
      await onSelectSoundfont(sf.filename);
      if (isMountedRef.current) {
        setIsLoading(false);
      }
      if (onClose) {
        onClose();
      }
      (document.activeElement as HTMLElement)?.blur?.();
    } catch (err) {
      if (isMountedRef.current) {
        setIsLoading(false);
      }
      console.error(err);
    }
  };

  const handleTriggerImport = async () => {
    if (!onImportSoundfont || isImporting || isLoading) return;
    try {
      setIsImporting(true);
      await onImportSoundfont();
    } catch (err) {
      console.error("Error al importar soundfont:", err);
    } finally {
      setIsImporting(false);
    }
  };

  const handleDelete = async (e: React.MouseEvent, sf: SoundFontInfo) => {
    e.stopPropagation();
    if (!onDeleteSoundfont || deletingFilename || isLoading) return;
    try {
      setDeletingFilename(sf.filename);
      await onDeleteSoundfont(sf.filename);
    } catch (err) {
      console.error("Error al eliminar soundfont:", err);
    } finally {
      setDeletingFilename(null);
    }
  };

  const getInstrumentConfig = (sf: SoundFontInfo) => {
    const fn = sf.filename.toLowerCase();
    if (sf.isUser) {
      return {
        title: sf.displayName || sf.filename.replace(".sf2", ""),
        subtitle: `${sf.presets?.length || 1} Preset${(sf.presets?.length || 1) > 1 ? "s" : ""} • ${sf.fileSizeMb.toFixed(1)} MB`,
        accentBorder: "border-emerald-400/90 shadow-[0_0_30px_rgba(16,185,129,0.3)]",
        activeLed: "bg-emerald-400 shadow-[0_0_8px_#10b981]",
        imgSrc: null,
        glowColor: "rgba(16, 185, 129, 0.45)",
      };
    } else if (fn.includes("piano") && !fn.includes("rhodes")) {
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

  const carouselRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(true);

  const checkScrollBounds = () => {
    if (!carouselRef.current) return;
    const { scrollLeft, scrollWidth, clientWidth } = carouselRef.current;
    setCanScrollLeft(scrollLeft > 10);
    setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 10);
  };

  const scrollByAmount = (direction: "left" | "right") => {
    if (!carouselRef.current) return;
    const scrollOffset = 320;
    carouselRef.current.scrollBy({
      left: direction === "left" ? -scrollOffset : scrollOffset,
      behavior: "smooth",
    });
  };

  useEffect(() => {
    if (!isOpen) return;

    checkScrollBounds();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isInitialSplash && onClose && !isLoading) {
        onClose();
      } else if (e.key === "ArrowLeft") {
        scrollByAmount("left");
      } else if (e.key === "ArrowRight") {
        scrollByAmount("right");
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, isInitialSplash, onClose, isLoading]);

  // Manejo fluido de rueda del mouse horizontal sin ser frenado por scroll-snap nativo
  useEffect(() => {
    if (!isOpen) return;
    const el = carouselRef.current;
    if (!el) return;

    let snapTimeout: number | null = null;

    const onWheelHandler = (e: WheelEvent) => {
      // Si la rueda es vertical o diagonal, convertir a desplazamiento horizontal
      const delta = Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
      if (delta !== 0) {
        e.preventDefault();
        // Desactivar temporalmente scroll-snap para evitar pelea de frames en WebKitGTK
        el.style.scrollSnapType = "none";
        el.style.scrollBehavior = "auto";
        el.scrollLeft += delta * 1.35;
        checkScrollBounds();

        if (snapTimeout !== null) {
          window.clearTimeout(snapTimeout);
        }
        snapTimeout = window.setTimeout(() => {
          if (el) {
            el.style.scrollBehavior = "";
            el.style.scrollSnapType = "";
          }
        }, 160);
      }
    };

    el.addEventListener("wheel", onWheelHandler, { passive: false });
    return () => {
      el.removeEventListener("wheel", onWheelHandler);
      if (snapTimeout !== null) {
        window.clearTimeout(snapTimeout);
      }
    };
  }, [isOpen, availableSoundfonts.length]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 bg-[#040406] text-white flex flex-col select-none overflow-hidden isolate">
      {/* Fondo de Estudio Inmersivo: Micro-malla acústica y Spotlight Cenital (aislado en capa GPU fija) */}
      <div
        className="absolute inset-0 pointer-events-none z-0 transform-gpu"
        style={{ contain: "strict", backfaceVisibility: "hidden" }}
      >
        <div
          className="absolute inset-0"
          style={{
            backgroundImage: "radial-gradient(circle, rgba(255, 255, 255, 0.11) 1.2px, transparent 1.2px)",
            backgroundSize: "28px 28px",
          }}
        />
        <div
          className="absolute inset-0"
          style={{
            background:
              "radial-gradient(ellipse 90% 80% at 50% 50%, transparent 45%, #040406 96%), radial-gradient(ellipse 90% 60% at 50% 0%, rgba(255, 255, 255, 0.08) 0%, transparent 70%)",
          }}
        />
      </div>

      {/* Encabezado Superior de Pantalla Completa */}
      <header className="relative z-10 px-6 sm:px-10 py-5 flex items-center justify-between border-b border-white/[0.08] bg-[#060609]/95 shrink-0 transform-gpu">
        <div className="flex items-center gap-3.5">
          <span className="w-2.5 h-2.5 rounded-full bg-emerald-400 shadow-[0_0_10px_#10b981]" />
          <div>
            <h1 className="text-sm sm:text-base font-mono font-bold tracking-widest text-zinc-100 uppercase">
              GALERÍA DE INSTRUMENTOS
            </h1>
            <p className="text-[11px] font-mono text-zinc-400">
              Selecciona o importa un SoundFont (.sf2) para cargar sus capas y muestras en memoria
            </p>
          </div>
        </div>

        <div className="flex items-center gap-4">
          <div className="hidden sm:flex items-center gap-2 px-3 py-1 rounded-full bg-white/[0.04] border border-white/[0.08] text-[11px] font-mono text-zinc-400">
            <Music size={12} className="text-emerald-400" />
            <span>{availableSoundfonts.length} instrumentos disponibles</span>
          </div>

          {!isInitialSplash && onClose && !isLoading && (
            <button
              type="button"
              onClick={(e) => {
                e.currentTarget.blur();
                onClose();
              }}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl border border-white/[0.1] bg-white/[0.05] hover:bg-white/[0.12] hover:border-white/20 text-zinc-300 hover:text-white text-xs font-mono transition-colors cursor-pointer shadow-sm"
              title="Cerrar galería (ESC)"
            >
              <span>ESC</span>
              <X size={15} />
            </button>
          )}
        </div>

        {/* Micro-línea de progreso de carga de muestras anclada al borde inferior del header */}
        {isLoading && (
          <div className="absolute bottom-0 inset-x-0 h-0.5 bg-zinc-800 overflow-hidden">
            <div className="h-full w-full bg-gradient-to-r from-emerald-500 via-cyan-400 to-amber-400 animate-pulse shadow-[0_0_8px_rgba(34,211,238,0.8)]" />
          </div>
        )}
      </header>

      {/* Carrusel Horizontal de Instrumentos de Estudio (Fila Única Centrada con Scroll Suave) */}
      <main className="relative z-10 flex-1 flex flex-col justify-center items-center px-4 sm:px-8 overflow-hidden">
        {/* Contenedor relativo para posicionar flechas flotantes de navegación */}
        <div className="relative w-full max-w-[1400px] flex items-center">
          {/* Botón Flecha Izquierda */}
          <button
            type="button"
            onClick={() => scrollByAmount("left")}
            disabled={!canScrollLeft}
            className={`absolute -left-3 sm:-left-5 z-30 w-11 h-11 rounded-full border border-white/[0.12] bg-zinc-950/95 flex items-center justify-center text-zinc-300 hover:text-white hover:border-emerald-400/50 hover:bg-zinc-900 transition-[transform,opacity,border-color,background-color,color] duration-200 transform-gpu shadow-[0_4px_20px_rgba(0,0,0,0.8)] cursor-pointer ${
              !canScrollLeft ? "opacity-0 pointer-events-none" : "opacity-90 hover:opacity-100 hover:scale-105"
            }`}
            title="Anterior (Flecha izquierda)"
          >
            <ChevronLeft size={22} />
          </button>

          {/* Track del Carrusel Horizontal */}
          <div
            ref={carouselRef}
            onScroll={checkScrollBounds}
            className="w-full flex items-center gap-5 sm:gap-6 overflow-x-auto py-8 px-6 sm:px-12 no-scrollbar scroll-smooth snap-x snap-proximity"
            style={{
              scrollbarWidth: "none",
              msOverflowStyle: "none",
            }}
          >
            {/* 1. Tarjeta Importar SoundFont (.sf2) */}
            <div
              onClick={handleTriggerImport}
              style={{ backfaceVisibility: "hidden" }}
              className={`group shrink-0 snap-center relative w-64 sm:w-72 h-[340px] rounded-2xl border-2 border-dashed transition-[transform,border-color,background-color] duration-200 ease-out transform-gpu will-change-transform flex flex-col items-center justify-center p-6 text-center cursor-pointer overflow-hidden ${
                isImporting
                  ? "border-emerald-500/70 bg-emerald-950/20 shadow-[0_0_35px_rgba(16,185,129,0.25)]"
                  : "border-white/[0.14] hover:border-emerald-400/60 bg-black/50 hover:bg-black/70 hover:scale-[1.02]"
              }`}
            >
              {/* Resplandor reactivo verde esmeralda */}
              <div className="absolute inset-0 pointer-events-none opacity-0 group-hover:opacity-40 transition-opacity duration-300 transform-gpu bg-[radial-gradient(circle_at_50%_50%,rgba(16,185,129,0.35)_0%,transparent_70%)]" />

              <div className="relative z-10 flex flex-col items-center">
                <div className="w-16 h-16 rounded-2xl bg-zinc-900/90 border border-white/[0.1] flex items-center justify-center mb-4 shadow-[0_10px_25px_rgba(0,0,0,0.7)] group-hover:border-emerald-400/50 group-hover:scale-110 transition-[transform,border-color] duration-200 transform-gpu">
                  {isImporting ? (
                    <UploadCloud size={28} className="text-emerald-400 animate-pulse" />
                  ) : (
                    <Plus size={30} className="text-zinc-300 group-hover:text-emerald-400 transition-colors" />
                  )}
                </div>
                <h3 className="text-sm font-mono font-bold uppercase tracking-wider text-zinc-100 group-hover:text-white transition-colors">
                  {isImporting ? "IMPORTANDO..." : "IMPORTAR SF2"}
                </h3>
                <p className="text-[11px] font-mono text-zinc-400 mt-2 max-w-[190px] leading-relaxed">
                  Añade tus propios archivos descargados (.sf2) a la biblioteca
                </p>
                <span className="mt-5 inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-white/[0.04] border border-white/[0.08] text-[10px] font-mono text-zinc-400 group-hover:text-emerald-300 group-hover:border-emerald-500/40 transition-colors">
                  <Music size={11} />
                  Explorador o Drag & Drop
                </span>
              </div>
            </div>

            {/* 2. Tarjetas de Instrumentos Disponibles */}
            {availableSoundfonts.map((sf) => {
              const config = getInstrumentConfig(sf);
              const isCurrent = currentSoundfont?.filename === sf.filename;
              const isSelected = selectedFilename === sf.filename;
              const isDeleting = deletingFilename === sf.filename;

              return (
                <div
                  key={sf.filename}
                  onClick={() => !isLoading && !isDeleting && handleSelect(sf)}
                  style={{ backfaceVisibility: "hidden", contain: "layout paint" }}
                  className={`group shrink-0 snap-center relative w-64 sm:w-72 h-[340px] rounded-2xl border overflow-hidden transition-[transform,border-color,background-color] duration-200 ease-out transform-gpu will-change-transform flex flex-col justify-between p-5 cursor-pointer ${
                    isSelected
                      ? `${config.accentBorder} bg-black/95 scale-[1.02]`
                      : "border-white/[0.09] bg-black/70 hover:border-white/35 hover:bg-black/85 hover:scale-[1.01]"
                  }`}
                >
                  {/* Underglow reactivo */}
                  <div
                    className={`absolute inset-0 pointer-events-none transition-opacity duration-300 transform-gpu z-0 ${
                      isSelected ? "opacity-80" : "opacity-0 group-hover:opacity-45"
                    }`}
                    style={{
                      background: `radial-gradient(circle at 50% 38%, ${config.glowColor} 0%, transparent 68%)`,
                    }}
                  />

                  {/* Imagen o Showcase del Instrumento */}
                  <div className="absolute top-3 inset-x-0 h-48 flex items-center justify-center p-2 pointer-events-none z-10">
                    {config.imgSrc ? (
                      <div className="relative w-full h-full flex items-center justify-center transform-gpu will-change-transform group-hover:scale-105 transition-transform duration-200 ease-out">
                        {/* Sombra ambiental pre-calculada por gradiente (evita recalcular drop-shadow gaussiano sobre alfa en cada frame en WebKitGTK) */}
                        <div
                          className="absolute inset-x-6 bottom-2 h-10 rounded-full pointer-events-none"
                          style={{
                            background: "radial-gradient(ellipse at center, rgba(0,0,0,0.92) 0%, rgba(0,0,0,0.5) 50%, transparent 78%)",
                          }}
                        />
                        <img
                          src={config.imgSrc}
                          alt={config.title}
                          loading="eager"
                          decoding="async"
                          draggable={false}
                          style={{ backfaceVisibility: "hidden" }}
                          className="relative z-10 w-full h-full object-contain transform-gpu"
                        />
                      </div>
                    ) : (
                      <div className="w-24 h-24 rounded-2xl bg-zinc-900/90 border border-emerald-500/30 flex flex-col items-center justify-center p-3 shadow-[0_12px_24px_rgba(0,0,0,0.8)] group-hover:border-emerald-400 group-hover:scale-105 transition-[transform,border-color] duration-200 transform-gpu">
                        <Music size={38} className="text-emerald-400/90 mb-1" />
                        <span className="text-[9px] font-mono text-emerald-300 font-bold uppercase tracking-wider">
                          USER SF2
                        </span>
                      </div>
                    )}
                  </div>

                  {/* Gradiente de contraste en base */}
                  <div className="absolute bottom-0 inset-x-0 h-36 bg-gradient-to-t from-black via-black/90 to-transparent pointer-events-none z-20" />

                  {/* Badges superiores y botón borrar */}
                  <div className="relative z-30 flex items-center justify-between w-full">
                    {sf.isUser ? (
                      <span className="px-2 py-0.5 rounded-full bg-emerald-950/80 border border-emerald-500/40 text-[9px] font-mono text-emerald-300 font-bold tracking-wider">
                        CUSTOM
                      </span>
                    ) : (
                      <span />
                    )}

                    <div className="flex items-center gap-1.5">
                      {sf.isUser && onDeleteSoundfont && (
                        <button
                          type="button"
                          onClick={(e) => handleDelete(e, sf)}
                          disabled={isDeleting || isCurrent}
                          title={isCurrent ? "No se puede eliminar mientras está en uso" : "Eliminar SoundFont"}
                          className="p-1 rounded-md text-zinc-500 hover:text-rose-400 hover:bg-rose-950/40 transition-colors disabled:opacity-30 disabled:hover:text-zinc-500 cursor-pointer"
                        >
                          <Trash2 size={13} />
                        </button>
                      )}
                      {isCurrent && (
                        <span className="flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-zinc-950/90 border border-emerald-500/50 text-[9.5px] font-mono text-emerald-300 font-bold tracking-wider shadow-[0_0_12px_rgba(16,185,129,0.35)]">
                          <span className={`w-1.5 h-1.5 rounded-full ${config.activeLed}`} />
                          ACTIVO
                        </span>
                      )}
                    </div>
                  </div>

                  {/* Pie de tarjeta con info y acción */}
                  <div className="relative z-30 flex flex-col justify-end">
                    <h3
                      className="text-base font-extrabold text-white tracking-tight truncate"
                      style={{ textShadow: "0 2px 4px rgba(0,0,0,0.95)" }}
                      title={config.title}
                    >
                      {config.title}
                    </h3>
                    <p
                      className="text-[11px] font-mono text-zinc-300/90 truncate mt-0.5 font-medium"
                      style={{ textShadow: "0 1px 2px rgba(0,0,0,0.95)" }}
                    >
                      {config.subtitle}
                    </p>

                    <div className="mt-3 pt-2.5 border-t border-white/[0.12] flex items-center justify-between text-[11px] font-mono">
                      <span
                        className={
                          isCurrent
                            ? "text-emerald-400 flex items-center gap-1.5 font-bold"
                            : isSelected
                            ? "text-zinc-100 font-bold"
                            : "text-zinc-300 group-hover:text-white font-medium transition-colors"
                        }
                        style={isCurrent ? { textShadow: "0 0 8px rgba(16,185,129,0.4)" } : undefined}
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
                        <ArrowRight size={13} className="text-zinc-400 group-hover:text-white group-hover:translate-x-1 transition-[transform,color] duration-200 transform-gpu" />
                      )}
                    </div>
                  </div>
                </div>
              );
            })}

            {/* Spacer derecho para garantizar que la última tarjeta quede 100% visible sin corte */}
            <div className="w-8 shrink-0 pointer-events-none" />
          </div>

          {/* Botón Flecha Derecha */}
          <button
            type="button"
            onClick={() => scrollByAmount("right")}
            disabled={!canScrollRight}
            className={`absolute -right-3 sm:-right-5 z-30 w-11 h-11 rounded-full border border-white/[0.12] bg-zinc-950/95 flex items-center justify-center text-zinc-300 hover:text-white hover:border-emerald-400/50 hover:bg-zinc-900 transition-[transform,opacity,border-color,background-color,color] duration-200 transform-gpu shadow-[0_4px_20px_rgba(0,0,0,0.8)] cursor-pointer ${
              !canScrollRight ? "opacity-0 pointer-events-none" : "opacity-90 hover:opacity-100 hover:scale-105"
            }`}
            title="Siguiente (Flecha derecha)"
          >
            <ChevronRight size={22} />
          </button>
        </div>
      </main>
    </div>
  );
};
