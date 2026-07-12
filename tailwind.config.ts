import type { Config } from 'tailwindcss';

const config: Config = {
  darkMode: ['class'],
  content: [
    './index.html',
    './src/**/*.{ts,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        // QueryBen / Sqlair product family palette — do NOT use arbitrary values
        jade: {
          DEFAULT: '#1A2E2A',
          50: '#E8F0EE',
          100: '#C5D9D5',
          200: '#9BBFB8',
          300: '#70A49B',
          400: '#4D8F85',
          500: '#2A7A6E',
          600: '#1A2E2A', // primary
          700: '#162623',
          800: '#111D1B',
          900: '#0B1312',
        },
        cream: {
          DEFAULT: '#F4EFE7',
          50: '#FDFCFA',
          100: '#F9F7F2',
          200: '#F4EFE7', // bg
          300: '#EBE3D5',
          400: '#DDD3C0',
        },
        amber: {
          DEFAULT: '#D58A4A',
          50: '#FBF2E9',
          100: '#F4DFC3',
          200: '#EDCB9C',
          300: '#E6B776',
          400: '#DFA050',
          500: '#D58A4A', // accent
          600: '#B8722E',
          700: '#8F571F',
        },
        // Semantic aliases wired into shadcn CSS vars pattern
        background: 'hsl(var(--background))',
        foreground: 'hsl(var(--foreground))',
        card: {
          DEFAULT: 'hsl(var(--card))',
          foreground: 'hsl(var(--card-foreground))',
        },
        popover: {
          DEFAULT: 'hsl(var(--popover))',
          foreground: 'hsl(var(--popover-foreground))',
        },
        primary: {
          DEFAULT: 'hsl(var(--primary))',
          foreground: 'hsl(var(--primary-foreground))',
        },
        secondary: {
          DEFAULT: 'hsl(var(--secondary))',
          foreground: 'hsl(var(--secondary-foreground))',
        },
        muted: {
          DEFAULT: 'hsl(var(--muted))',
          foreground: 'hsl(var(--muted-foreground))',
        },
        accent: {
          DEFAULT: 'hsl(var(--accent))',
          foreground: 'hsl(var(--accent-foreground))',
        },
        destructive: {
          DEFAULT: 'hsl(var(--destructive))',
          foreground: 'hsl(var(--destructive-foreground))',
        },
        border: 'hsl(var(--border))',
        input: 'hsl(var(--input))',
        ring: 'hsl(var(--ring))',
      },
      fontFamily: {
        sans: ['Geist', 'system-ui', 'sans-serif'],
        mono: ['GeistMono', 'ui-monospace', 'monospace'],
      },
      borderRadius: {
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
      },
      keyframes: {
        'accordion-down': {
          from: { height: '0' },
          to: { height: 'var(--radix-accordion-content-height)' },
        },
        'accordion-up': {
          from: { height: 'var(--radix-accordion-content-height)' },
          to: { height: '0' },
        },
        'fade-in': {
          from: { opacity: '0', transform: 'translateY(4px)' },
          to: { opacity: '1', transform: 'translateY(0)' },
        },
      },
      animation: {
        'accordion-down': 'accordion-down 0.2s ease-out',
        'accordion-up': 'accordion-up 0.2s ease-out',
        'fade-in': 'fade-in 0.15s ease-out',
      },
    },
  },
  plugins: [],
} satisfies Config;

export default config;
