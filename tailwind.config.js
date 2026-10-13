/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ['"Geist Sans"', '-apple-system', 'BlinkMacSystemFont', '"Segoe UI"', 'Roboto', 'sans-serif'],
        mono: ['"Geist Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'monospace'],
      },
      colors: {
        studio: {
          950: "#090a0f",
          900: "#10121a",
          850: "#151822",
          800: "#1b1f2c",
          700: "#2a3044",
          600: "#3d4560",
          accent: "#ff3e3e",
          cyan: "#00e5ff",
          amber: "#ffb300",
        },
      },
    },
  },
  plugins: [],
};
