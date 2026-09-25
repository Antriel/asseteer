const STORAGE_KEY = 'asseteer-settings';

/** What the audio player does when a track reaches its end. */
export type AudioEndMode = 'stop' | 'next' | 'repeat';

interface PersistedSettings {
  preGenerateThumbnails: boolean;
  audioEndMode: AudioEndMode;
}

const DEFAULTS: PersistedSettings = {
  preGenerateThumbnails: false,
  audioEndMode: 'stop',
};

function loadFromStorage(): PersistedSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULTS, ...JSON.parse(raw) };
  } catch {}
  return { ...DEFAULTS };
}

class Settings {
  preGenerateThumbnails = $state(DEFAULTS.preGenerateThumbnails);
  audioEndMode = $state<AudioEndMode>(DEFAULTS.audioEndMode);

  constructor() {
    const stored = loadFromStorage();
    this.preGenerateThumbnails = stored.preGenerateThumbnails;
    this.audioEndMode = stored.audioEndMode;
  }

  setPreGenerateThumbnails(value: boolean) {
    this.preGenerateThumbnails = value;
    this.#save();
  }

  setAudioEndMode(value: AudioEndMode) {
    this.audioEndMode = value;
    this.#save();
  }

  #save() {
    const data: PersistedSettings = {
      preGenerateThumbnails: this.preGenerateThumbnails,
      audioEndMode: this.audioEndMode,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
  }
}

export const settings = new Settings();
