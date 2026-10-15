import { useEffect, useRef, useState, useCallback } from "react";

export const QWERTY_BASE_NOTE = 60; // C3
export const QWERTY_VELOCITY = 127;
export const MIN_OCTAVE_OFFSET = -2;
export const MAX_OCTAVE_OFFSET = 2;

export const QWERTY_OCTAVE_DOWN = "KeyZ";
export const QWERTY_OCTAVE_UP = "KeyX";

export const QWERTY_NOTE_OFFSETS: Record<string, number> = {
  KeyA: 0, // C
  KeyW: 1, // C#
  KeyS: 2, // D
  KeyE: 3, // D#
  KeyD: 4, // E
  KeyF: 5, // F
  KeyT: 6, // F#
  KeyG: 7, // G
  KeyY: 8, // G#
  KeyH: 9, // A
  KeyU: 10, // A#
  KeyJ: 11, // B
  KeyK: 12, // C (+1)
  KeyO: 13, // C# (+1)
  KeyL: 14, // D (+1)
  KeyP: 15, // D# (+1)
  Semicolon: 16, // E (+1)
};

export const QWERTY_PAD_CODES: Record<string, number> = {
  Digit1: 1,
  Digit2: 2,
  Digit3: 3,
  Digit4: 4,
  Digit5: 5,
  Digit6: 6,
  Digit7: 7,
  Digit8: 8,
};

export interface KeyRange {
  minNote: number;
  maxNote: number;
  label?: string;
}

export interface UseQwertyKeyboardOptions {
  enabled: boolean;
  workstationMode: "keys" | "seq";
  keyRange?: KeyRange;
  onNoteOn: (note: number, velocity: number) => void;
  onNoteOff: (note: number) => void;
  onPadOn: (padIdx: number) => void;
  onPadOff: (padIdx: number) => void;
}

interface ActiveKeyEntry {
  type: "note" | "pad";
  value: number; // número de nota o índice de pad (1..8)
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!target || !(target instanceof HTMLElement)) {
    return false;
  }

  // Si el elemento quedó huérfano (desconectado del DOM), no bloquear
  if (!target.isConnected) {
    return false;
  }

  const tagName = target.tagName.toUpperCase();
  if (tagName === "INPUT" || tagName === "TEXTAREA") {
    return true;
  }

  if (target.isContentEditable) {
    return true;
  }

  // Si está dentro de un modal interactivo conectado al DOM
  const modalContainer = target.closest("dialog, [role='dialog'], [data-modal]");
  if (modalContainer && modalContainer.isConnected) {
    return true;
  }

  return false;
}

export function useQwertyKeyboard({
  enabled,
  workstationMode,
  keyRange,
  onNoteOn,
  onNoteOff,
  onPadOn,
  onPadOff,
}: UseQwertyKeyboardOptions) {
  const [octaveOffset, setOctaveOffset] = useState<number>(0);

  // Registro de teclas físicas activas para evitar notas colgadas al cambiar de octava
  const activeKeysRef = useRef<Map<string, ActiveKeyEntry>>(new Map());

  // Callbacks actualizados en ref para no recrear los listeners de window
  const callbacksRef = useRef({ onNoteOn, onNoteOff, onPadOn, onPadOff });
  callbacksRef.current = { onNoteOn, onNoteOff, onPadOn, onPadOff };

  // Ref del estado de octava y rango para acceso sincronizado en event listeners
  const stateRef = useRef({ octaveOffset, keyRange, workstationMode, enabled });
  stateRef.current = { octaveOffset, keyRange, workstationMode, enabled };

  const releaseAll = useCallback(() => {
    if (activeKeysRef.current.size === 0) return;

    activeKeysRef.current.forEach((entry) => {
      if (entry.type === "note") {
        callbacksRef.current.onNoteOff(entry.value);
      } else if (entry.type === "pad") {
        callbacksRef.current.onPadOff(entry.value);
      }
    });

    activeKeysRef.current.clear();
  }, []);

  // Liberar todo si se desactiva QWERTY o cambia el modo de workstation
  useEffect(() => {
    if (!enabled) {
      releaseAll();
    }
  }, [enabled, releaseAll]);

  useEffect(() => {
    releaseAll();
  }, [workstationMode, releaseAll]);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.repeat || e.ctrlKey || e.altKey || e.metaKey) {
        return;
      }

      if (isTypingTarget(e.target)) {
        return;
      }

      // Evitar disparo duplicado si la tecla ya está registrada como activa
      if (activeKeysRef.current.has(e.code)) {
        return;
      }

      // Ajuste de octava
      if (e.code === QWERTY_OCTAVE_DOWN) {
        setOctaveOffset((prev) => Math.max(MIN_OCTAVE_OFFSET, prev - 1));
        e.preventDefault();
        return;
      }

      if (e.code === QWERTY_OCTAVE_UP) {
        setOctaveOffset((prev) => Math.min(MAX_OCTAVE_OFFSET, prev + 1));
        e.preventDefault();
        return;
      }

      // Pads rítmicos (1 a 8) - activos en modo keys y modo seq
      if (e.code in QWERTY_PAD_CODES) {
        const padIdx = QWERTY_PAD_CODES[e.code];
        activeKeysRef.current.set(e.code, { type: "pad", value: padIdx });
        callbacksRef.current.onPadOn(padIdx);
        e.preventDefault();
        return;
      }

      // Notas melódicas - solo activas en modo keys
      if (stateRef.current.workstationMode === "keys" && e.code in QWERTY_NOTE_OFFSETS) {
        const semitoneOffset = QWERTY_NOTE_OFFSETS[e.code];
        const note = QWERTY_BASE_NOTE + stateRef.current.octaveOffset * 12 + semitoneOffset;

        // Comprobación de límites MIDI globales
        if (note < 0 || note > 127) {
          return;
        }

        // Comprobación de rango acústico del instrumento (ej. Mellotron 43-77)
        const currentKeyRange = stateRef.current.keyRange;
        if (currentKeyRange) {
          if (note < currentKeyRange.minNote || note > currentKeyRange.maxNote) {
            return;
          }
        }

        activeKeysRef.current.set(e.code, { type: "note", value: note });
        callbacksRef.current.onNoteOn(note, QWERTY_VELOCITY);
        e.preventDefault();
        return;
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      // Si la tecla liberada estaba activa, procesarla independientemente del target
      const activeEntry = activeKeysRef.current.get(e.code);
      if (!activeEntry) {
        return;
      }

      activeKeysRef.current.delete(e.code);

      if (activeEntry.type === "note") {
        callbacksRef.current.onNoteOff(activeEntry.value);
      } else if (activeEntry.type === "pad") {
        callbacksRef.current.onPadOff(activeEntry.value);
      }

      e.preventDefault();
    };

    const handleWindowBlur = () => {
      releaseAll();
    };

    const handleVisibilityChange = () => {
      if (document.hidden) {
        releaseAll();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleWindowBlur);
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleWindowBlur);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      releaseAll();
    };
  }, [enabled, releaseAll]);

  return {
    octaveOffset,
    setOctaveOffset,
    baseNote: QWERTY_BASE_NOTE + octaveOffset * 12,
  };
}
