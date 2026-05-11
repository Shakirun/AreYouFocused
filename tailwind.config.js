/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        surface: "#F0FDFA",
        ink: "#134E4A",
        brand: {
          DEFAULT: "#0D9488",
          hover: "#0f766e",
          muted: "#14B8A6",
        },
        action: {
          DEFAULT: "#F97316",
          hover: "#ea580c",
        },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
      },
      transitionDuration: {
        interaction: "200ms",
      },
    },
  },
  plugins: [],
};
