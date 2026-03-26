import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}", "./lib/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ds: {
          bg: {
            100: "#0a0a0a",
            200: "#111111",
          },
          gray: {
            100: "#1a1a1a",
            200: "#1f1f1f",
            300: "#292929",
            400: "#2e2e2e",
            500: "#454545",
            600: "#5e5e5e",
            700: "#6e6e6e",
            800: "#7c7c7c",
            900: "#a0a0a0",
            1000: "#ededed",
            "alpha-100": "rgba(255,255,255,0.03)",
            "alpha-200": "rgba(255,255,255,0.06)",
            "alpha-400": "rgba(255,255,255,0.10)",
          },
        },
        blue: {
          700: "#0070f3",
          900: "#52a8ff",
        },
        green: {
          700: "#0cce6b",
          900: "#52e78c",
        },
        amber: {
          700: "#f5a623",
          900: "#ffcc4d",
        },
        red: {
          700: "#e5484d",
          900: "#ff6369",
        },
      },
      fontFamily: {
        sans: ["Geist Sans Variable", "system-ui", "sans-serif"],
        mono: ["Geist Mono Variable", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};

export default config;
