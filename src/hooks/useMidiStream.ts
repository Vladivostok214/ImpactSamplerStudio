import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { MidiPortInfo, NormalizedMidiEvent } from "../types/midi";

export function useMidiStream() {
  const [events, setEvents] = useState<NormalizedMidiEvent[]>([]);
  const [ports, setPorts] = useState<MidiPortInfo[]>([]);
  const [activeNotes, setActiveNotes] = useState<Map<number, number>>(new Map()); // note -> velocity
  const [activePads, setActivePads] = useState<Map<number, number>>(new Map()); // padIndex (1-8) -> velocity
  const [faders, setFaders] = useState<number[]>([64, 64, 64, 64, 64, 64, 64, 64, 100]); // F1-F9 (0-127)
  const [knobs, setKnobs] = useState<number[]>([64, 64, 64, 64, 64, 64, 64, 64]); // K1-K8 (0-127)
  const [pitchBend, setPitchBend] = useState<number>(0); // -8192 to 8191
  const [modWheel, setModWheel] = useState<number>(0); // 0 to 127
  const [connected, setConnected] = useState<boolean>(false);

  useEffect(() => {
    // 1. Cargar lista de puertos
    invoke<MidiPortInfo[]>("list_midi_ports")
      .then((p) => {
        setPorts(p);
        if (p.length > 0) setConnected(true);
      })
      .catch(console.error);

    // 2. Escuchar stream de eventos MIDI en tiempo real desde Rust
    let unlistenFn: (() => void) | null = null;
    let isSubscribed = true;

    listen<NormalizedMidiEvent>("midi-event", (eventPayload) => {
      const midiEvent = eventPayload.payload;

      setEvents((prev) => [midiEvent, ...prev.slice(0, 49)]);

      const { event } = midiEvent;
      const ev = event as any;
      const eventType = (event.type || "").toLowerCase();

      if (eventType === "noteon") {
        setActiveNotes((prev) => {
          const next = new Map(prev);
          next.set(ev.note, ev.velocity);
          return next;
        });

        if (ev.padIndex) {
          setActivePads((prev) => {
            const next = new Map(prev);
            next.set(ev.padIndex, ev.velocity);
            return next;
          });
        }
      } else if (eventType === "noteoff") {
        setActiveNotes((prev) => {
          const next = new Map(prev);
          next.delete(ev.note);
          return next;
        });

        if (ev.padIndex) {
          setActivePads((prev) => {
            const next = new Map(prev);
            next.delete(ev.padIndex);
            return next;
          });
        }
      } else if (eventType === "controlchange") {
        const controller = ev.controller;
        const value = ev.value;
        // Mapeo Faders
        const faderMap: Record<number, number> = {
          // Bloque correlativo Nektar LX49+ (F1 - F9)
          38: 0, 39: 1, 40: 2, 41: 3, 42: 4, 43: 5, 44: 6, 45: 7, 46: 8,
          // Preset alternativo General MIDI
          73: 0, 75: 1, 72: 2, 91: 3, 92: 4, 93: 5, 94: 6, 95: 7, 7: 8,
        };
        if (faderMap[controller] !== undefined) {
          setFaders((prev) => {
            const next = [...prev];
            next[faderMap[controller]] = value;
            return next;
          });
        }

        // Mapeo Knobs
        const knobMap: Record<number, number> = {
          // Bloque correlativo Nektar LX49+ (K1 - K8)
          56: 0, 57: 1, 58: 2, 59: 3, 60: 4, 61: 5, 62: 6, 63: 7,
          // Preset alternativo General MIDI
          74: 0, 71: 1, 5: 2, 84: 3, 78: 4, 76: 5, 77: 6, 10: 7,
        };
        if (knobMap[controller] !== undefined) {
          setKnobs((prev) => {
            const next = [...prev];
            next[knobMap[controller]] = value;
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
        unlistenFn = unlisten;
      }
    });

    return () => {
      isSubscribed = false;
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, []);

  const refreshPorts = async () => {
    try {
      const p = await invoke<MidiPortInfo[]>("list_midi_ports");
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

  return {
    events,
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
  };
}
