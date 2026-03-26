import { SvelteMap } from 'svelte/reactivity';

// Scan progress details
export interface ScanProgress {
  phase: 'idle' | 'discovering' | 'scanning' | 'inserting' | 'indexing' | 'complete';
  filesFound: number;
  filesInserted: number;
  filesTotal: number;
  zipsScanned: number;
  currentPath: string | null;
}

// Per-folder active scan state
export interface ActiveScan {
  folderPath: string;
  startedAt: number;
  progressMessage: string;
  details: ScanProgress;
}

// UI state for scan operations and feedback
class UIState {
  // Active scans keyed by normalized folder path
  activeScans = new SvelteMap<string, ActiveScan>();

  // Derived: true if any scan is in progress
  get isScanning(): boolean {
    return this.activeScans.size > 0;
  }

  // For StatusBar: pick the earliest-started active scan's details
  get scanDetails(): ScanProgress {
    if (this.activeScans.size === 0) {
      return {
        phase: 'idle',
        filesFound: 0,
        filesInserted: 0,
        filesTotal: 0,
        zipsScanned: 0,
        currentPath: null,
      };
    }
    let earliest: ActiveScan | null = null;
    for (const scan of this.activeScans.values()) {
      if (!earliest || scan.startedAt < earliest.startedAt) earliest = scan;
    }
    return earliest!.details;
  }

  // For StatusBar: earliest scan start time
  get scanStartedAt(): number | null {
    if (this.activeScans.size === 0) return null;
    let earliest = Infinity;
    for (const scan of this.activeScans.values()) {
      if (scan.startedAt < earliest) earliest = scan.startedAt;
    }
    return earliest;
  }

  // Start tracking a new scan
  startScan(folderPath: string) {
    this.activeScans.set(folderPath, {
      folderPath,
      startedAt: Date.now(),
      progressMessage: 'Starting scan...',
      details: {
        phase: 'idle',
        filesFound: 0,
        filesInserted: 0,
        filesTotal: 0,
        zipsScanned: 0,
        currentPath: null,
      },
    });
  }

  // Update progress for a specific scan
  updateScan(folderPath: string, progressMessage: string, details: ScanProgress) {
    const scan = this.activeScans.get(folderPath);
    if (scan) {
      this.activeScans.set(folderPath, { ...scan, progressMessage, details });
    }
  }

  // Stop tracking a scan
  endScan(folderPath: string) {
    this.activeScans.delete(folderPath);
  }

  // Check if a specific folder is being scanned
  isScanningFolder(folderPath: string): boolean {
    return this.activeScans.has(folderPath);
  }

  // Toast notifications
  toasts = $state<Toast[]>([]);

  // Confirm dialog
  confirm = $state<ConfirmRequest | null>(null);
}

export interface ConfirmRequest {
  message: string;
  title: string;
  confirmLabel: string;
  resolve: (value: boolean) => void;
}

// Toast notification types
export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface Toast {
  id: number;
  message: string;
  type: ToastType;
}

let toastId = 0;

// Export singleton instance
export const uiState = new UIState();

/**
 * Show a toast notification
 */
export function showToast(message: string, type: ToastType = 'info', duration: number = 4000) {
  const id = ++toastId;
  const toast: Toast = { id, message, type };

  uiState.toasts = [...uiState.toasts, toast];

  // Auto-remove after duration
  setTimeout(() => {
    dismissToast(id);
  }, duration);

  return id;
}

/**
 * Dismiss a toast notification
 */
export function dismissToast(id: number) {
  uiState.toasts = uiState.toasts.filter((t) => t.id !== id);
}

/**
 * Show a confirm dialog. Returns true if confirmed, false if cancelled.
 */
export function showConfirm(
  message: string,
  title: string = 'Confirm',
  confirmLabel: string = 'Confirm',
): Promise<boolean> {
  return new Promise((resolve) => {
    uiState.confirm = { message, title, confirmLabel, resolve };
  });
}

export function resolveConfirm(value: boolean) {
  const req = uiState.confirm;
  uiState.confirm = null;
  req?.resolve(value);
}
