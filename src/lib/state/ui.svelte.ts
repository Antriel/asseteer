// UI state for scan operations and feedback
class UIState {
  isScanning = $state(false);
  scanProgress = $state('');
  currentSessionId = $state<number | null>(null);
}

// Export singleton instance
export const uiState = new UIState();
