// UI state for scan operations and feedback
class UIState {
  isScanning = $state(false);
  scanProgress = $state('');
  currentSessionId = $state<number | null>(null);

  // Processing state
  isProcessing = $state(false);
  processProgress = $state('');

  // Toast notifications
  toasts = $state<Toast[]>([]);
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
  uiState.toasts = uiState.toasts.filter(t => t.id !== id);
}
