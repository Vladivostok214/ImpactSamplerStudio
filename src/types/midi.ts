export interface MidiPortInfo {
  index: number;
  name: string;
  isDefault: boolean;
  isConnected: boolean;
}

export interface MidiStatusEvent {
  connected: boolean;
  portName?: string | null;
  ports?: MidiPortInfo[];
}

export type MidiEventType =
  | { type: "NoteOn"; note: number; velocity: number; noteName: string; padIndex?: number }
  | { type: "NoteOff"; note: number; velocity: number; noteName: string; padIndex?: number }
  | { type: "ControlChange"; controller: number; value: number; label?: string }
  | { type: "PitchBend"; value: number; normalized: number };

export interface NormalizedMidiEvent {
  portName: string;
  channel: number;
  event: MidiEventType;
  timestampMs: number;
}

export interface AudioDeviceInfo {
  id: string;
  name: string;
  isDefault: boolean;
  sampleRates: number[];
  channels?: number;
}

export interface AudioConfig {
  deviceId: string | null;
  bufferSize: number;
  sampleRate: number;
  exclusive: boolean;
  masterVolume: number;
}
