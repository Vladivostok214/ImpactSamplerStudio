export type ArpMode = "up" | "down" | "upDown";
export type ArpRate = "quarter" | "eighth" | "sixteenth" | "thirtySecond";

export interface ArpStateInfo {
  enabled: boolean;
  hold: boolean;
  mode: ArpMode;
  rate: ArpRate;
  octaves: number;
}
