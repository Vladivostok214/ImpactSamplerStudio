export type UIActionId =
  | "play"
  | "stop"
  | "record"
  | "tapTempo"
  | "bpmDown"
  | "bpmUp"
  | "click"
  | "visualMetronome"
  | "bank"
  | "grid"
  | "page"
  | "undo"
  | "clearTrack"
  | "clearPattern"
  | "fxRouteKeys"
  | "fxRoutePads"
  | "workstationMode"
  | "toggleArp"
  | "toggleArpHold";

export interface ActionBinding {
  messageType: "cc" | "note";
  number: number;
}

export type MidiLearnTarget =
  | { type: "fader"; index: number } // 0-7: Pad gains, 8: Master volume
  | { type: "knob"; index: number }  // 0-7: FX / Matrix Knobs
  | { type: "pad"; index: number }   // 0-7: Drum Pads
  | { type: "action"; id: UIActionId }; // 17 acciones UI mapeables

export interface MidiLearnMapping {
  faders: Record<number, number>; // index (0..8) -> CC number
  knobs: Record<number, number>;  // index (0..7) -> CC number
  pads: Record<number, number>;   // index (0..7) -> MIDI Note (0..127)
  actions: Record<string, ActionBinding>; // UIActionId -> { messageType, number }
}

export const DEFAULT_MIDI_LEARN_MAPPING: MidiLearnMapping = {
  faders: {
    0: 38,
    1: 39,
    2: 40,
    3: 41,
    4: 42,
    5: 43,
    6: 44,
    7: 45,
    8: 46, // Master Volume (F9)
  },
  knobs: {
    0: 56, // Cutoff
    1: 57, // Res
    2: 58, // Drive
    3: 59, // Reverb
    4: 60, // D-Time
    5: 61, // D-Fdbk
    6: 62, // D-Mix
    7: 63, // Pan
  },
  pads: {
    0: 36, // C1 (Kick)
    1: 38, // D1 (Snare)
    2: 42, // F#1 (Closed Hat)
    3: 46, // A#1 (Open Hat)
    4: 39, // D#1 (Clap)
    5: 45, // A1 (Low Tom)
    6: 48, // C2 (High Tom)
    7: 49, // C#2 (Crash / Perc)
  },
  actions: {
    play: { messageType: "cc", number: 106 },
    stop: { messageType: "cc", number: 105 },
    record: { messageType: "cc", number: 107 },
    tapTempo: { messageType: "cc", number: 102 },
    bpmDown: { messageType: "cc", number: 103 },
    bpmUp: { messageType: "cc", number: 104 },
    click: { messageType: "cc", number: 47 },       // B1 (LX49+)
    grid: { messageType: "cc", number: 48 },        // B2 (LX49+)
    bank: { messageType: "cc", number: 51 },        // B5 (LX49+)
    undo: { messageType: "cc", number: 52 },        // B6 (LX49+)
    clearTrack: { messageType: "cc", number: 53 },  // B7 (LX49+)
    visualMetronome: { messageType: "cc", number: 54 }, // B8 (LX49+)
    clearPattern: { messageType: "cc", number: 55 },    // B9 (LX49+)
  },
};

const STORAGE_KEY = "impact_sampler_midi_learn_v1";

export function loadSavedMidiMapping(): MidiLearnMapping {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_MIDI_LEARN_MAPPING;
    const parsed = JSON.parse(raw);
    return {
      faders: { ...DEFAULT_MIDI_LEARN_MAPPING.faders, ...(parsed.faders || {}) },
      knobs: { ...DEFAULT_MIDI_LEARN_MAPPING.knobs, ...(parsed.knobs || {}) },
      pads: { ...DEFAULT_MIDI_LEARN_MAPPING.pads, ...(parsed.pads || {}) },
      actions: { ...DEFAULT_MIDI_LEARN_MAPPING.actions, ...(parsed.actions || {}) },
    };
  } catch {
    return DEFAULT_MIDI_LEARN_MAPPING;
  }
}

export function saveMidiMapping(mapping: MidiLearnMapping): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(mapping));
  } catch (err) {
    console.error("Error guardando mapeo MIDI Learn:", err);
  }
}

export function getMidiNoteName(note: number): string {
  const names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
  const octave = Math.floor(note / 12) - 1;
  const name = names[note % 12];
  return `${name}${octave}`;
}

export function getActionMappingLabel(mapping: MidiLearnMapping | undefined, actionId: UIActionId): string | null {
  const b = mapping?.actions?.[actionId];
  if (!b) return null;
  if (b.messageType === "cc") return `CC ${b.number}`;
  return `Nota ${getMidiNoteName(b.number)} (#${b.number})`;
}
