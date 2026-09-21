/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        retro: {
          bg: '#0b0f19',
          card: '#111827',
          cardHover: '#1f2937',
          border: '#374151',
          accent: '#06b6d4',
          glow: '#3b82f6',
        },
      },
    },
  },
  plugins: [],
}
