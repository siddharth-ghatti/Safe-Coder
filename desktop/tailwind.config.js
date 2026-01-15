/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        // OpenCode-inspired dark theme
        background: "#0d0d0d",
        foreground: "#e5e5e5",
        muted: "#262626",
        "muted-foreground": "#a3a3a3",
        border: "#2e2e2e",
        card: "#171717",
        "card-foreground": "#e5e5e5",
        primary: "#22c55e",
        "primary-foreground": "#0d0d0d",
        secondary: "#404040",
        "secondary-foreground": "#e5e5e5",
        accent: "#3b82f6",
        "accent-foreground": "#e5e5e5",
        destructive: "#ef4444",
        "destructive-foreground": "#e5e5e5",
        success: "#22c55e",
        warning: "#f59e0b",
        // Diff colors
        "diff-add": "#22c55e",
        "diff-add-bg": "rgba(34, 197, 94, 0.1)",
        "diff-remove": "#ef4444",
        "diff-remove-bg": "rgba(239, 68, 68, 0.1)",
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "Menlo", "Monaco", "monospace"],
      },
    },
  },
  plugins: [],
};
