"use client";

import { useEffect, useState } from "react";

export function Nav() {
  const [dark, setDark] = useState(false);

  useEffect(() => {
    const stored = localStorage.getItem("theme");
    const prefersDark =
      stored === "dark" ||
      (!stored && window.matchMedia("(prefers-color-scheme: dark)").matches);
    setDark(prefersDark);
    document.documentElement.classList.toggle("dark", prefersDark);
  }, []);

  const toggle = () => {
    const next = !dark;
    setDark(next);
    document.documentElement.classList.toggle("dark", next);
    localStorage.setItem("theme", next ? "dark" : "light");
  };

  return (
    <nav className="border-b border-(--color-border) bg-(--color-bg-secondary) px-6 py-3 flex items-center justify-between">
      <a href="/" className="text-xl font-bold text-(--color-accent)">
        🎵 ruscore
      </a>
      <button
        onClick={toggle}
        className="p-2 rounded-lg hover:bg-(--color-bg-tertiary) transition-colors"
        aria-label="Toggle dark mode"
      >
        {dark ? "☀️" : "🌙"}
      </button>
    </nav>
  );
}
