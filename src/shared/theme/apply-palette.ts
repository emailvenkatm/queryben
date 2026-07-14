import type { Palette, PaletteColors } from './presets';

// Writes every color in palette.colors to a CSS custom property on <html>.
// camelCase key `bgElevated` becomes `--color-bg-elevated`.
export function applyPalette(palette: Palette): void {
  const root = document.documentElement;
  (Object.entries(palette.colors) as Array<[keyof PaletteColors, string]>).forEach(
    ([key, val]) => {
      const cssVar = '--color-' + key.replace(/([A-Z])/g, '-$1').toLowerCase();
      root.style.setProperty(cssVar, val);
    },
  );
}
