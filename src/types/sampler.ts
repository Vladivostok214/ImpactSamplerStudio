export type PadPlaybackMode = "oneShot" | "gate";
export type PadEngineMode = "wavSampler" | "drumKitSf2";
export type FxRouting = "off" | "keys" | "both" | "pads";

export interface PadSlotInfo {
  index: number;
  padNumber: number;
  sampleName: string | null;
  mode: PadPlaybackMode;
  gain: number;
  hasSample: boolean;
}

export type KnobTarget =
  | "cutoff"
  | "resonance"
  | "drive"
  | "reverbMix"
  | "delayTime"
  | "delayFeedback"
  | "delayMix"
  | "chorusMix"
  | "pan";

export interface KnobMappingInfo {
  index: number;
  target: KnobTarget;
  label: string;
  displayName: string;
  value: number;
}

export type StepResolution = "eighth" | "sixteenth" | "thirtySecond";
export type QuantizeMode = "off" | "eighth" | "sixteenth" | "thirtySecond";
export type PatternBank = "a" | "b";

export interface SequencerStateInfo {
  isPlaying: boolean;
  isPaused: boolean;
  isRecording: boolean;
  bpm: number;
  currentStep: number;
  resolution: StepResolution;
  currentBank: PatternBank;
  isClickEnabled: boolean;
  selectedPad: number;
  quantizeMode: QuantizeMode;
  quantizeStrength: number;
  latencyCompensationTicks: number;
  pattern: number[][]; // 8 pads x 32 steps (velocities 0..127)
}

export interface SequencerStepEvent {
  step: number;
  isPlaying: boolean;
  bpm: number;
  beat: number;
  maxBeats: number;
}

