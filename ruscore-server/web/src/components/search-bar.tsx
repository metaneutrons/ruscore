"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { fetchSuggestions } from "@/lib/api";
import { Suggestion } from "@/lib/types";

export function SearchBar({
  onSearch,
}: {
  onSearch: (q: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [open, setOpen] = useState(false);
  const router = useRouter();
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    clearTimeout(timerRef.current);
    if (!query.trim()) {
      setSuggestions([]);
      return;
    }
    timerRef.current = setTimeout(async () => {
      const results = await fetchSuggestions(query);
      setSuggestions(results);
      setOpen(results.length > 0);
    }, 300);
    return () => clearTimeout(timerRef.current);
  }, [query]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      )
        setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      setOpen(false);
      onSearch(query);
    }
  };

  return (
    <div ref={containerRef} className="relative">
      <input
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={handleKeyDown}
        onFocus={() => suggestions.length > 0 && setOpen(true)}
        placeholder="Search scores…"
        className="w-full px-4 py-2 rounded-lg border border-(--color-border) bg-(--color-bg) text-(--color-text) placeholder:text-(--color-text-secondary) focus:outline-none focus:ring-2 focus:ring-(--color-accent)"
      />
      {open && (
        <ul className="absolute z-10 mt-1 w-full rounded-lg border border-(--color-border) bg-(--color-bg) shadow-lg max-h-60 overflow-auto">
          {suggestions.map((s) => (
            <li key={s.id}>
              <button
                className="w-full text-left px-4 py-2 hover:bg-(--color-bg-tertiary) transition-colors"
                onMouseDown={() => {
                  setOpen(false);
                  router.push(`/jobs/detail?id=${s.id}`);
                }}
              >
                <span className="font-medium">{s.title}</span>
                {s.composer && (
                  <span className="text-(--color-text-secondary) ml-2 text-sm">
                    — {s.composer}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
