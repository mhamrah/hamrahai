import plugin from "tailwindcss/plugin";
import type { Config } from "tailwindcss";
import animate from "tailwindcss-animate";

export default {
  content: ["./src/**/*.{js,ts,jsx,tsx,mdx}"],
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      borderWidth: {
        base: "var(--border-width)",
        DEFAULT: "calc(var(--border-width) + 1px)",
        2: "calc(var(--border-width) + 2px)",
        4: "calc(var(--border-width) + 4px)",
        8: "calc(var(--border-width) + 8px)",
      },
      boxShadow: {
        base: "var(--shadow-base)",
        sm: "var(--shadow-sm)",
        DEFAULT: "var(--shadow)",
        md: "var(--shadow-md)",
        lg: "var(--shadow-lg)",
        xl: "var(--shadow-xl)",
        "2xl": "var(--shadow-2xl)",
        inner: "var(--shadow-inner)",
      },
      strokeWidth: {
        0: "0",
        base: "var(--stroke-width)",
        1: "calc(var(--stroke-width) + 1px)",
        2: "calc(var(--stroke-width) + 2px)",
      },
      colors: {
        // Custom color palette
        'hunyadi-yellow': {
          DEFAULT: '#f6bd60',
          100: '#412904',
          200: '#815308',
          300: '#c27c0b',
          400: '#f2a11f',
          500: '#f6bd60',
          600: '#f8ca80',
          700: '#f9d7a0',
          800: '#fbe4bf',
          900: '#fdf2df',
        },
        'linen': {
          DEFAULT: '#f7ede2',
          100: '#4a3014',
          200: '#956129',
          300: '#cf904e',
          400: '#e3bf99',
          500: '#f7ede2',
          600: '#f9f1e9',
          700: '#faf4ee',
          800: '#fcf8f4',
          900: '#fdfbf9',
        },
        'tea-rose': {
          DEFAULT: '#f5cac3',
          100: '#4b150d',
          200: '#962a19',
          300: '#db432c',
          400: '#e88677',
          500: '#f5cac3',
          600: '#f7d4ce',
          700: '#f9deda',
          800: '#fbe9e7',
          900: '#fdf4f3',
        },
        'cambridge-blue': {
          DEFAULT: '#84a59d',
          100: '#192220',
          200: '#324440',
          300: '#4b665f',
          400: '#65887f',
          500: '#84a59d',
          600: '#9cb6b0',
          700: '#b5c8c4',
          800: '#cedbd7',
          900: '#e6edeb',
        },
        'light-coral': {
          DEFAULT: '#f28482',
          100: '#430807',
          200: '#87100e',
          300: '#ca1815',
          400: '#eb423f',
          500: '#f28482',
          600: '#f59d9b',
          700: '#f7b5b4',
          800: '#facecd',
          900: '#fce6e6',
        },

        // Semantic color mappings
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [
    require("tailwindcss-animate"),
    plugin(function ({ addUtilities }) {
      addUtilities({
        ".press": {
          transform: "var(--transform-press)",
        },
      });
    }),
    animate,
  ],
} satisfies Config;
