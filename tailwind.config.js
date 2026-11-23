/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ['"Plus Jakarta Sans"', "system-ui", "-apple-system", "Segoe UI", "sans-serif"],
        mono: ['"JetBrains Mono"', '"Cascadia Mono"', "Consolas", "monospace"],
      },
      colors: {
        obsidian: {
          950: "#000000",
          900: "#060608",
          850: "#0B0B0F",
          800: "#121218",
        },
      },
    },
  },
  plugins: [],
};
