export interface SoundFontPresetInfo {
  index: number;
  name: string;
  bank: number;
  patch: number;
}

export interface SoundFontInfo {
  id: string;
  filename: string;
  displayName: string;
  description: string;
  category: string;
  fileSizeMb: number;
  presets: SoundFontPresetInfo[];
  isUser?: boolean;
}
