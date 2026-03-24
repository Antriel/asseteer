// Scan progress details
export interface ScanProgress {
  phase: 'idle' | 'discovering' | 'scanning' | 'inserting' | 'complete';
  filesFound: number;
  filesInserted: number;
  filesTotal: number;
  zipsScanned: number;
  currentPath: string | null;
}

// UI state for scan operations and feedback
class UIState {
  isScanning = $state(false);
  scanProgress = $state('');
  scanningFolderPath = $state<string | null>(null);
  currentSessionId = $state<number | null>(null);

  // Detailed scan progress (persists across navigation)
  scanDetails = $state<ScanProgress>({
    phase: 'idle',
    filesFound: 0,
    filesInserted: 0,
    filesTotal: 0,
    zipsScanned: 0,
    currentPath: null,
  });

  // Toast notifications
  toasts = $state<Toast[]>([]);

  // Confirm dialog
  confirm = $state<ConfirmRequest | null>(null);

  // Reset scan details
  resetScanDetails() {
    this.scanDetails = {
      phase: 'idle',
      filesFound: 0,
      filesInserted: 0,
      filesTotal: 0,
      zipsScanned: 0,
      currentPath: null,
    };
  }
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
