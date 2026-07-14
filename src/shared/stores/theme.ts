import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { applyPalette } from '@/shared/theme/apply-palette';
import {
  DEFAULT_PALETTE_ID,
  getFallbackPreset,
  getPresetById,
  type Palette,
} from '@/shared/theme/presets';

interface ThemeState {
  paletteId: string;
  customPalette?: Palette;
  hasFileOverride: boolean;

  setPreset: (id: string) => void;
  setCustomPalette: (palette: Palette, fromFile: boolean) => void;
  clearCustomPalette: () => void;
  getActivePalette: () => Palette;
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      paletteId: DEFAULT_PALETTE_ID,
      customPalette: undefined,
      hasFileOverride: false,

      setPreset: (id) => {
        const preset = getPresetById(id) ?? getFallbackPreset();
        // Picking a preset clears any file/custom override.
        set({ paletteId: preset.id, customPalette: undefined, hasFileOverride: false });
        applyPalette(preset);
      },

      setCustomPalette: (palette, fromFile) => {
        set({ paletteId: palette.id, customPalette: palette, hasFileOverride: fromFile });
        applyPalette(palette);
      },

      clearCustomPalette: () => {
        const state = get();
        const fallbackId =
          state.customPalette?.id === state.paletteId ? DEFAULT_PALETTE_ID : state.paletteId;
        const preset = getPresetById(fallbackId) ?? getFallbackPreset();
        set({ paletteId: preset.id, customPalette: undefined, hasFileOverride: false });
        applyPalette(preset);
      },

      getActivePalette: () => {
        const state = get();
        if (state.customPalette) return state.customPalette;
        return getPresetById(state.paletteId) ?? getFallbackPreset();
      },
    }),
    {
      name: 'queryben.theme.v3',
      // Don't persist file-override state — theme.json is re-read on startup.
      partialize: (state) => ({
        paletteId: state.paletteId,
        customPalette: state.hasFileOverride ? undefined : state.customPalette,
      }),
      onRehydrateStorage: () => (state) => {
        if (!state) return;
        const active =
          state.customPalette ?? getPresetById(state.paletteId) ?? getFallbackPreset();
        applyPalette(active);
      },
    },
  ),
);
