"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

export function Nav() {
  const [dark, setDark] = useState(false);

  useEffect(() => {
    setDark(document.documentElement.classList.contains("dark"));
  }, []);

  const toggle = () => {
    const next = !dark;
    setDark(next);
    document.documentElement.classList.toggle("dark", next);
    localStorage.theme = next ? "dark" : "light";
  };

  return (
    <nav className="border-b border-(--color-border) bg-(--color-bg-secondary)">
      <div className="mx-auto flex max-w-5xl items-center justify-between px-4 py-3">
        <div className="flex items-center gap-6">
          <Link href="/" className="text-lg font-bold text-(--color-accent)">
            Ruscore
          </Link>
          <Link href="/jobs" className="text-sm text-(--color-text-secondary) hover:text-(--color-text)">
            Jobs
          </Link>
        </div>
        <button onClick={toggle} className="rounded-md p-2 text-(--color-text-secondary) hover:bg-(--color-bg-tertiary)" aria-label="Toggle dark mode">
          {dark ? "☀️" : "🌙"}
        </button>
      </div>
    </nav>
  );
}
