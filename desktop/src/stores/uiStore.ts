import { create } from "zustand";

interface UIState {
  // Layout
  sidebarCollapsed: boolean;
  changesPanelCollapsed: boolean;
  terminalOpen: boolean;

  // Selected file for diff view
  selectedDiffFile: string | null;

  // Modal states
  isNewSessionModalOpen: boolean;
  isSettingsModalOpen: boolean;

  // Actions
  toggleSidebar: () => void;
  toggleChangesPanel: () => void;
  toggleTerminal: () => void;
  setSelectedDiffFile: (path: string | null) => void;
  setNewSessionModalOpen: (open: boolean) => void;
  setSettingsModalOpen: (open: boolean) => void;
}

export const useUIStore = create<UIState>((set) => ({
  // Initial state
  sidebarCollapsed: false,
  changesPanelCollapsed: false,
  terminalOpen: false,
  selectedDiffFile: null,
  isNewSessionModalOpen: false,
  isSettingsModalOpen: false,

  // Actions
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  toggleChangesPanel: () => set((state) => ({ changesPanelCollapsed: !state.changesPanelCollapsed })),
  toggleTerminal: () => set((state) => ({ terminalOpen: !state.terminalOpen })),
  setSelectedDiffFile: (path) => set({ selectedDiffFile: path }),
  setNewSessionModalOpen: (open) => set({ isNewSessionModalOpen: open }),
  setSettingsModalOpen: (open) => set({ isSettingsModalOpen: open }),
}));
