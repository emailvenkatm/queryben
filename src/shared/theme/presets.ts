export type PaletteColors = {
  bg: string;
  bgElevated: string;
  bgSidebar: string;
  primary: string;
  primaryHover: string;
  accent: string;
  accentHover: string;
  text: string;
  textMuted: string;
  textInverse: string;
  border: string;
  error: string;
  success: string;
  warning: string;
  codeBg: string;
};

export type Palette = {
  id: string;
  name: string;
  description: string;
  colors: PaletteColors;
};

export const PRESETS: Palette[] = [
  {
    id: 'silhouette-umber',
    name: 'Silhouette Umber & Terracotta',
    description: '',
    colors: {
      bg: '#EEDFC8',
      bgElevated: '#F8EDD9',
      bgSidebar: '#3C2A22',
      primary: '#3C2A22',
      primaryHover: '#5A3E34',
      accent: '#C46A3C',
      accentHover: '#A5522A',
      text: '#2A1D17',
      textMuted: '#6F5445',
      textInverse: '#EEDFC8',
      border: '#D8C6AC',
      error: '#8D3A2E',
      success: '#5D6E3B',
      warning: '#C46A3C',
      codeBg: '#DFC9AB',
    },
  },
];

export const DEFAULT_PALETTE_ID = 'silhouette-umber';

export function getFallbackPreset(): Palette {
  const preset = PRESETS.find((p) => p.id === DEFAULT_PALETTE_ID);
  return preset ?? (PRESETS[0] as Palette);
}

export function getPresetById(id: string): Palette | undefined {
  return PRESETS.find((p) => p.id === id);
}

export function mergePalette(base: Palette, patch: Partial<Palette>): Palette {
  return {
    id: patch.id ?? base.id,
    name: patch.name ?? base.name,
    description: patch.description ?? base.description,
    colors: { ...base.colors, ...(patch.colors ?? {}) },
  };
}

export function coercePalette(raw: unknown): Palette | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const r = raw as Record<string, unknown>;
  const colors = r.colors as Record<string, unknown> | undefined;
  if (!colors || typeof colors !== 'object') return null;

  const knownKeys: Array<keyof PaletteColors> = [
    'bg', 'bgElevated', 'bgSidebar', 'primary', 'primaryHover',
    'accent', 'accentHover', 'text', 'textMuted', 'textInverse',
    'border', 'error', 'success', 'warning', 'codeBg',
  ];
  const validColors: Partial<PaletteColors> = {};
  for (const key of knownKeys) {
    const v = colors[key];
    if (typeof v === 'string' && /^#[0-9A-Fa-f]{3,8}$/.test(v)) {
      validColors[key] = v;
    }
  }
  if (Object.keys(validColors).length === 0) return null;

  const base = getFallbackPreset();
  return {
    id: typeof r.id === 'string' ? r.id : 'custom',
    name: typeof r.name === 'string' ? r.name : 'Custom palette',
    description: typeof r.description === 'string' ? r.description : 'Loaded from theme.json',
    colors: { ...base.colors, ...validColors },
  };
}
