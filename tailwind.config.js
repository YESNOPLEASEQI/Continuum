/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ["Segoe UI Variable", "Segoe UI", "system-ui", "sans-serif"],
        mono: ["Cascadia Code", "SFMono-Regular", "Consolas", "monospace"],
      },
      colors: {
        ink: "#0b0e14",
        panel: "#11161f",
        raised: "#171d28",
        line: "#2a3342",
        muted: "#8b96a8",
        text: "#dce4ef",
        signal: "#65aefc",
        amber: "#e6ad4f",
        danger: "#e36d72",
        success: "#62c59b"
      },
      boxShadow: {
        focus: "0 0 0 2px #0b0e14, 0 0 0 4px #65aefc",
      },
    },
  },
  plugins: [],
};
