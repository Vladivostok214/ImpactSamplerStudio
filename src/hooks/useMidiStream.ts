import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { MidiPortInfo, NormalizedMidiEvent, MidiStatusEvent } from "../types/midi";
import {
  MidiLearnTarget,
  MidiLearnMapping,
  DEFAULT_MIDI_LEARN_MAPPING,
  loadSavedMidiMapping,
  saveMidiMapping,
} from "../types/midiLearn";

export function useMidiStream() {
  const [events, setEvents] = useState<NormalizedMidiEvent[]>([]);
  const [ports, setPorts] = useState<MidiPortInfo[]>([]);
  const [activeNotes, setActiveNotes] = useState<Map<number, number>>(new Map()); // note -> velocity
  const [activePads, setActivePads] = useState<Map<number, number>>(new Map()); // padIndex (1-8) -> velocity
  const [faders, setFaders] = useState<number[]>([64, 64, 64, 64, 64, 64, 64, 64, 100]); // F1-F9 (0-127, F9 Master al 80%)
  const [knobs, setKnobs] = useState<number[]>([64, 64, 0, 0, 0, 0, 0, 64]); // K1-K8
  const [pitchBend, setPitchBend] = useState<number>(0); // -8192 to 8191
  const [modWheel, setModWheel] = useState<number>(0); // 0 to 127
  const [connected, setConnected] = useState<boolean>(false);

  // Estados de MIDI Learn
  const [isMidiLearnActive, setIsMidiLearnActive] = useState<boolean>(false);
  const [selectedLearnTarget, setSelectedLearnTarget] = useState<MidiLearnTarget | null>(null);
  const [midiLearnMapping, setMidiLearnMapping] = useState<MidiLearnMapping>(loadSavedMidiMapping);
  const [lastLearnedInfo, setLastLearnedInfo] = useState<{ label: string; detail: string } | null>(null);

  // Ref de sincronización para evitar cierres obsoletos en el listener de Tauri
  const learnStateRef = useRef({
    isActive: isMidiLearnActive,
    target: selectedLearnTarget,
    mapping: midiLearnMapping,
  });

  useEffect(() => {
    learnStateRef.current = {
      isActive: isMidiLearnActive,
      target: selectedLearnTarget,
      mapping: midiLearnMapping,
    };
  }, [isMidiLearnActive, selectedLearnTarget, midiLearnMapping]);

  // Mapeo entre IDs de TypeScript y enum SequencerAction de Rust
  const ACTION_ID_TO_RUST: Record<string, string> = {
    play: "togglePlay",
    stop: "stop",
    record: "toggleRecord",
    tapTempo: "tapTempo",
    bpmDown: "bpmDown",
    bpmUp: "bpmUp",
    click: "toggleClick",
    visualMetronome: "toggleVisualMetronome",
    bank: "toggleBank",
    grid: "cycleResolution",
    page: "advancePage",
    undo: "undo",
    clearTrack: "clearSelectedTrack",
    clearPattern: "clearAll",
    fxRouteKeys: "fxRouteKeys",
    fxRoutePads: "fxRoutePads",
    workstationMode: "toggleWorkstationMode",
  };

  // Sincronizar mapeos activos con el motor de audio en Rust
  const syncMappingsToRust = (mapping: MidiLearnMapping) => {
    const fadersArr = [0, 1, 2, 3, 4, 5, 6, 7, 8].map((i) => mapping.faders[i] ?? (i === 8 ? 46 : 38 + i));
    const knobsArr = [0, 1, 2, 3, 4, 5, 6, 7].map((i) => mapping.knobs[i] ?? 56 + i);
    const padsArr = [0, 1, 2, 3, 4, 5, 6, 7].map((i) => mapping.pads[i] ?? [36, 38, 42, 46, 39, 45, 48, 49][i]);
    invoke("set_midi_learn_mappings", {
      faders: fadersArr,
      knobs: knobsArr,
      pads: padsArr,
    }).catch(console.error);

    const actionBindings: Array<{ message_type: string; number: number; action: string }> = [];
    if (mapping.actions) {
      for (const [id, binding] of Object.entries(mapping.actions)) {
        const rustAction = ACTION_ID_TO_RUST[id];
        if (rustAction && binding) {
          actionBindings.push({
            message_type: binding.messageType,
            number: binding.number,
            action: rustAction,
          });
        }
      }
    }
    invoke("set_action_midi_mappings", { bindings: actionBindings }).catch(console.error);
  };

  // Sincronizar mapeos guardados al montar la aplicación
  useEffect(() => {
    syncMappingsToRust(midiLearnMapping);
  }, []);

  // Limpiar mensaje de notificación de aprendizaje después de 3.5 segundos
  useEffect(() => {
    if (lastLearnedInfo) {
      const t = setTimeout(() => setLastLearnedInfo(null), 3500);
      return () => clearTimeout(t);
    }
  }, [lastLearnedInfo]);

  useEffect(() => {
    // 1. Cargar lista de puertos con estado de conexión real
    invoke<MidiPortInfo[]>("list_midi_ports")
      .then((p) => {
        setPorts(p);
        setConnected(p.some((x) => x.isConnected));
      })
      .catch(console.error);

    let unlistenMidiFn: (() => void) | null = null;
    let unlistenStatusFn: (() => void) | null = null;
    let isSubscribed = true;

    // 2. Escuchar cambios de estado del hardware en caliente (Hot-Plug Watchdog)
    listen<MidiStatusEvent>("midi-status-changed", (eventPayload) => {
      const { connected: isConn, ports: updatedPorts } = eventPayload.payload;
      setConnected(isConn);
      if (updatedPorts && updatedPorts.length > 0) {
        setPorts(updatedPorts);
      } else if (!isConn) {
        setPorts((prev) => prev.map((p) => ({ ...p, isConnected: false })));
      }
    }).then((unlisten) => {
      if (!isSubscribed) {
        unlisten();
      } else {
        unlistenStatusFn = unlisten;
      }
    });

    // 3. Escuchar stream de eventos MIDI en tiempo real desde Rust
    listen<NormalizedMidiEvent>("midi-event", (eventPayload) => {
      const midiEvent = eventPayload.payload;
      setEvents((prev) => [midiEvent, ...prev.slice(0, 49)]);

      const { event } = midiEvent;
      const ev = event as any;
      const eventType = (event.type || "").toLowerCase();

      const { isActive, target, mapping } = learnStateRef.current;

      // --- MODO MIDI LEARN: CAPTURA ACTIVA DE MENSAJE MIDI ---
      if (isActive && target) {
        if (target.type === "fader" && eventType === "controlchange") {
          const cc = ev.controller;
          const idx = target.index;
          setMidiLearnMapping((prev) => {
            const next = { ...prev, faders: { ...prev.faders, [idx]: cc } };
            saveMidiMapping(next);
            syncMappingsToRust(next);
            return next;
          });
          setLastLearnedInfo({
            label: idx === 8 ? "Master Volume (F9)" : `Fader ${idx + 1}`,
            detail: `Asignado a CC ${cc}`,
          });
          setSelectedLearnTarget(null);
          return;
        }

        if (target.type === "knob" && eventType === "controlchange") {
          const cc = ev.controller;
          const idx = target.index;
          setMidiLearnMapping((prev) => {
            const next = { ...prev, knobs: { ...prev.knobs, [idx]: cc } };
            saveMidiMapping(next);
            syncMappingsToRust(next);
            return next;
          });
          setLastLearnedInfo({
            label: `Knob ${idx + 1}`,
            detail: `Asignado a CC ${cc}`,
          });
          setSelectedLearnTarget(null);
          return;
        }

        if (target.type === "pad" && eventType === "noteon") {
          const note = ev.note;
          const idx = target.index;
          setMidiLearnMapping((prev) => {
            const next = { ...prev, pads: { ...prev.pads, [idx]: note } };
            saveMidiMapping(next);
            syncMappingsToRust(next);
            return next;
          });
          setLastLearnedInfo({
            label: `Pad ${idx + 1}`,
            detail: `Asignado a Nota ${ev.noteName || note} (#${note})`,
          });
          setSelectedLearnTarget(null);
          return;
        }

        if (target.type === "action" && (eventType === "controlchange" || eventType === "noteon")) {
          const isCc = eventType === "controlchange";
          const num = isCc ? ev.controller : ev.note;
          const msgType = isCc ? ("cc" as const) : ("note" as const);
          const actionId = target.id;

          const ACTION_LABELS: Record<string, string> = {
            play: "Play / Pause",
            stop: "Stop",
            record: "Record (Overdub)",
            tapTempo: "Tap Tempo",
            bpmDown: "BPM -1",
            bpmUp: "BPM +1",
            click: "Metrónomo Sonoro (Clic)",
            visualMetronome: "Metrónomo Visual (Overlay)",
            bank: "Banco A / B",
            grid: "Resolución Grilla",
            page: "Página (1/32)",
            undo: "Undo",
            clearTrack: "Borrar Pista Activa",
            clearPattern: "Borrar Todo el Patrón",
            fxRouteKeys: "FX Route: Teclado",
            fxRoutePads: "FX Route: Pads",
            workstationMode: "Vista Workstation (KEYS/SEQ)",
            toggleArp: "Arpegiador (On/Off)",
            toggleArpHold: "Arpegiador Hold (Latch)",
          };

          setMidiLearnMapping((prev) => {
            const nextActions = { ...(prev.actions || {}) };
            // Desvincular si este mismo mensaje ya estaba asignado a otra acción previa
            for (const [k, v] of Object.entries(nextActions)) {
              if (v && v.messageType === msgType && v.number === num) {
                delete nextActions[k];
              }
            }
            nextActions[actionId] = { messageType: msgType, number: num };
            const next = { ...prev, actions: nextActions };
            saveMidiMapping(next);
            syncMappingsToRust(next);
            return next;
          });

          setLastLearnedInfo({
            label: ACTION_LABELS[actionId] || actionId,
            detail: isCc ? `Asignado a CC ${num}` : `Asignado a Nota ${ev.noteName || num} (#${num})`,
          });
          setSelectedLearnTarget(null);
          return;
        }
      }

      // --- OPERACIÓN MUSICAL NORMAL (O CUANDO NO ESTÁ EN ESCUCHA ACTIVA) ---
      if (eventType === "noteon") {
        const vel = ev.velocity;
        if (vel > 0) {
          if (mapping.actions?.toggleArp?.messageType === "note" && mapping.actions.toggleArp.number === ev.note) {
            window.dispatchEvent(new CustomEvent("midi-toggle-arp"));
            return;
          }
          if (mapping.actions?.toggleArpHold?.messageType === "note" && mapping.actions.toggleArpHold.number === ev.note) {
            window.dispatchEvent(new CustomEvent("midi-toggle-arp-hold"));
            return;
          }
        }

        setActiveNotes((prev) => {
          const next = new Map(prev);
          next.set(ev.note, ev.velocity);
          return next;
        });

        // Buscar si la nota entrante corresponde a un pad mapeado
        let mappedPadNum = ev.padIndex;
        if (!mappedPadNum) {
          for (let pIdx = 0; pIdx < 8; pIdx++) {
            if (mapping.pads[pIdx] === ev.note) {
              mappedPadNum = pIdx + 1;
              break;
            }
          }
        }

        if (mappedPadNum) {
          setActivePads((prev) => {
            const next = new Map(prev);
            next.set(mappedPadNum, ev.velocity);
            return next;
          });
        }
      } else if (eventType === "noteoff") {
        if (mapping.actions?.toggleArp?.messageType === "note" && mapping.actions.toggleArp.number === ev.note) {
          return;
        }
        if (mapping.actions?.toggleArpHold?.messageType === "note" && mapping.actions.toggleArpHold.number === ev.note) {
          return;
        }

        setActiveNotes((prev) => {
          const next = new Map(prev);
          next.delete(ev.note);
          return next;
        });

        let mappedPadNum = ev.padIndex;
        if (!mappedPadNum) {
          for (let pIdx = 0; pIdx < 8; pIdx++) {
            if (mapping.pads[pIdx] === ev.note) {
              mappedPadNum = pIdx + 1;
              break;
            }
          }
        }

        if (mappedPadNum) {
          setActivePads((prev) => {
            const next = new Map(prev);
            next.delete(mappedPadNum);
            return next;
          });
        }
      } else if (eventType === "controlchange") {
        const controller = ev.controller;
        const value = ev.value;

        // Disparo de acciones de Arpegiador mapeadas por MIDI Learn
        if (value === undefined || value > 0) {
          if (mapping.actions?.toggleArp?.messageType === "cc" && mapping.actions.toggleArp.number === controller) {
            window.dispatchEvent(new CustomEvent("midi-toggle-arp"));
          }
          if (mapping.actions?.toggleArpHold?.messageType === "cc" && mapping.actions.toggleArpHold.number === controller) {
            window.dispatchEvent(new CustomEvent("midi-toggle-arp-hold"));
          }
        }

        // 1. Mapeo dinámico de Faders (con fallback a LX49+ de fábrica y GM)
        let matchedFaderIdx: number | null = null;
        for (let fIdx = 0; fIdx < 9; fIdx++) {
          if (mapping.faders[fIdx] === controller) {
            matchedFaderIdx = fIdx;
            break;
          }
        }
        if (matchedFaderIdx === null) {
          const defaultFaderFallback: Record<number, number> = {
            38: 0, 39: 1, 40: 2, 41: 3, 42: 4, 43: 5, 44: 6, 45: 7, 46: 8,
            73: 0, 75: 1, 72: 2, 91: 3, 92: 4, 93: 5, 94: 6, 95: 7, 7: 8,
          };
          if (defaultFaderFallback[controller] !== undefined) {
            matchedFaderIdx = defaultFaderFallback[controller];
          }
        }
        if (matchedFaderIdx !== null) {
          setFaders((prev) => {
            const next = [...prev];
            next[matchedFaderIdx!] = value;
            return next;
          });
        }

        // 2. Mapeo dinámico de Knobs (con fallback a LX49+ de fábrica y GM)
        let matchedKnobIdx: number | null = null;
        for (let kIdx = 0; kIdx < 8; kIdx++) {
          if (mapping.knobs[kIdx] === controller) {
            matchedKnobIdx = kIdx;
            break;
          }
        }
        if (matchedKnobIdx === null) {
          const defaultKnobFallback: Record<number, number> = {
            56: 0, 57: 1, 58: 2, 59: 3, 60: 4, 61: 5, 62: 6, 63: 7,
            74: 0, 71: 1, 5: 2, 84: 3, 78: 4, 76: 5, 77: 6, 10: 7,
          };
          if (defaultKnobFallback[controller] !== undefined) {
            matchedKnobIdx = defaultKnobFallback[controller];
          }
        }
        if (matchedKnobIdx !== null) {
          setKnobs((prev) => {
            const next = [...prev];
            next[matchedKnobIdx!] = value;
            return next;
          });
        }

        // Modulation Wheel
        if (controller === 1) {
          setModWheel(value);
        }
      } else if (eventType === "pitchbend") {
        setPitchBend(ev.normalized);
      }
    }).then((unlisten) => {
      if (!isSubscribed) {
        unlisten();
      } else {
        unlistenMidiFn = unlisten;
      }
    });

    return () => {
      isSubscribed = false;
      if (unlistenMidiFn) {
        unlistenMidiFn();
      }
      if (unlistenStatusFn) {
        unlistenStatusFn();
      }
    };
  }, []);

  const refreshPorts = async () => {
    try {
      const p = await invoke<MidiPortInfo[]>("rescan_midi_ports");
      setPorts(p);
      setConnected(p.some((x) => x.isConnected));
    } catch (err) {
      console.error(err);
    }
  };

  const selectPort = async (index: number) => {
    try {
      const updated = await invoke<MidiPortInfo[]>("select_midi_port", { index });
      setPorts(updated);
      setConnected(updated.some((x) => x.isConnected));
    } catch (err) {
      console.error(err);
    }
  };

  const triggerVirtualNote = (note: number, velocity: number, isOn: boolean) => {
    setActiveNotes((prev) => {
      const next = new Map(prev);
      if (isOn) {
        next.set(note, velocity);
      } else {
        next.delete(note);
      }
      return next;
    });
    invoke("trigger_virtual_note", {
      note,
      velocity: velocity / 127,
      isOn,
    }).catch(console.error);
  };

  const triggerVirtualPad = (padIdx: number, _note: number, isOn: boolean) => {
    const vel = isOn ? 127 : 0;
    setActivePads((prev) => {
      const next = new Map(prev);
      if (isOn) {
        next.set(padIdx, vel);
      } else {
        next.delete(padIdx);
      }
      return next;
    });
    invoke("trigger_virtual_pad", {
      padIndex: padIdx - 1,
      velocity: vel / 127,
      isOn,
    }).catch(console.error);
  };

  const triggerVirtualFader = (index: number, value: number) => {
    setFaders((prev) => {
      const next = [...prev];
      next[index] = value;
      return next;
    });
    const controller = index === 8 ? 46 : 38 + index;
    invoke("trigger_virtual_cc", {
      controller,
      value: value / 127,
    }).catch(console.error);
  };

  const triggerVirtualKnob = (index: number, value: number) => {
    setKnobs((prev) => {
      const next = [...prev];
      next[index] = value;
      return next;
    });
    const controller = 56 + index;
    invoke("trigger_virtual_cc", {
      controller,
      value: value / 127,
    }).catch(console.error);
  };

  const triggerVirtualPitchBend = (normalized: number) => {
    setPitchBend(normalized);
    invoke("trigger_virtual_pitch_bend", {
      normalized,
    }).catch(console.error);
  };

  const triggerVirtualModWheel = (value: number) => {
    setModWheel(value);
    invoke("trigger_virtual_cc", {
      controller: 1,
      value: value / 127,
    }).catch(console.error);
  };

  const clearEvents = () => {
    setEvents([]);
  };

  const toggleMidiLearn = () => {
    setIsMidiLearnActive((prev) => {
      const next = !prev;
      if (!next) {
        setSelectedLearnTarget(null);
      }
      return next;
    });
  };

  const resetMidiMappings = () => {
    setMidiLearnMapping(DEFAULT_MIDI_LEARN_MAPPING);
    saveMidiMapping(DEFAULT_MIDI_LEARN_MAPPING);
    syncMappingsToRust(DEFAULT_MIDI_LEARN_MAPPING);
    setLastLearnedInfo({
      label: "Restablecido",
      detail: "Valores de fábrica restaurados (Nektar LX49+)",
    });
  };

  return {
    events,
    clearEvents,
    ports,
    activeNotes,
    activePads,
    faders,
    knobs,
    pitchBend,
    modWheel,
    connected,
    refreshPorts,
    selectPort,
    triggerVirtualNote,
    triggerVirtualPad,
    triggerVirtualFader,
    triggerVirtualKnob,
    triggerVirtualPitchBend,
    triggerVirtualModWheel,
    // MIDI Learn
    isMidiLearnActive,
    toggleMidiLearn,
    selectedLearnTarget,
    setSelectedLearnTarget,
    midiLearnMapping,
    lastLearnedInfo,
    resetMidiMappings,
  };
}
