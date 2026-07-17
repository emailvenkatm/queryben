import { useEffect } from 'react';
import { commands } from '@/shared/api/tauri-bindings';
import { useThemeStore } from '@/shared/stores/theme';
import { coercePalette, getFallbackPreset, mergePalette } from '@/shared/theme/presets';
import { applyPalette } from '@/shared/theme/apply-palette';

// Reads theme.json from disk on mount and applies it if valid.
export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const { getActivePalette, setCustomPalette } = useThemeStore();

  useEffect(() => {
    commands.readThemeOverrideFile().then((json) => {
      if (!json) return;
      let raw: unknown;
      try {
        raw = JSON.parse(json);
      } catch {
        return;
      }
      const palette = coercePalette(raw);
      if (!palette) return;
      const base = getActivePalette();
      const merged = mergePalette(base, palette);
      setCustomPalette(merged, true);
    }).catch(() => {
      // theme.json is best-effort — failures fall back silently.
      applyPalette(getFallbackPreset());
    });
  }, []);

  return <>{children}</>;
}
