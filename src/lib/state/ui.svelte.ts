// UI state for scan operations and feedback
class UIState {
  isScanning = $state(false);
  scanProgress = $state('');
  currentSessionId = $state<number | null>(null);

  // Processing state
  isProcessing = $state(false);
  processProgress = $state('');
}

// Export singleton instance
export const uiState = new UIState();
