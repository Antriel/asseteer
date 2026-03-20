const STORAGE_KEY = 'asseteer-settings';

interface PersistedSettings {
  preGenerateThumbnails: boolean;
}

function loadFromStorage(): PersistedSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { preGenerateThumbnails: false, ...JSON.parse(raw) };
  } catch {}
  return { preGenerateThumbnails: false };
}

class Settings {
  preGenerateThumbnails = $state(false);

  constructor() {
    const stored = loadFromStorage();
    this.preGenerateThumbnails = stored.preGenerateThumbnails;
  }

  setPreGenerateThumbnails(value: boolean) {
    this.preGenerateThumbnails = value;
    this.#save();
  }

  #save() {
    const data: PersistedSettings = {
      preGenerateThumbnails: this.preGenerateThumbnails,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
  }
}

export const settings = new Settings();
