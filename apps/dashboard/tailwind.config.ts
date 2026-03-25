import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}", "./lib/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        cosmic: {
          purple: "#7C3AED",
          rose: "#F43F5E",
          dark: "#0F0B1A",
          surface: "#1A1425",
          border: "#2D2640",
          muted: "#6B5B8A",
          text: "#E8E0F0",
          bright: "#F5F0FF",
        },
      },
      fontFamily: {
        sans: ["Geist Sans Variable", "system-ui", "sans-serif"],
        mono: ["Geist Mono Variable", "ui-monospace", "monospace"],
      },
      backgroundImage: {
        "cosmic-gradient":
          "radial-gradient(ellipse at top left, rgba(124, 58, 237, 0.15) 0%, transparent 60%), radial-gradient(ellipse at bottom right, rgba(244, 63, 94, 0.10) 0%, transparent 60%)",
      },
      boxShadow: {
        "cosmic-sm": "0 1px 3px rgba(124, 58, 237, 0.12)",
        cosmic: "0 4px 16px rgba(124, 58, 237, 0.20)",
        "cosmic-lg": "0 8px 32px rgba(124, 58, 237, 0.28)",
      },
      borderRadius: {
        cosmic: "0.75rem",
      },
    },
  },
  plugins: [],
};

export default config;
