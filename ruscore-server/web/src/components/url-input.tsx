"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { createJob } from "@/lib/api";

export function UrlInput({ onSubmitted }: { onSubmitted?: () => void }) {
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const router = useRouter();

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;
    setLoading(true);
    setError("");
    try {
      const result = await createJob(url.trim());
      if (result.conflict) {
        router.push(`/jobs/detail?id=${result.id}`);
      } else {
        setUrl("");
        onSubmitted?.();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to submit");
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={submit} className="flex gap-2">
      <input
        type="url"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="Paste MuseScore URL…"
        required
        className="flex-1 px-4 py-3 rounded-lg border border-(--color-border) bg-(--color-bg) text-(--color-text) placeholder:text-(--color-text-secondary) focus:outline-none focus:ring-2 focus:ring-(--color-accent)"
      />
      <button
        type="submit"
        disabled={loading}
        className="px-6 py-3 rounded-lg bg-(--color-accent) text-white font-medium hover:bg-(--color-accent-hover) disabled:opacity-50 transition-colors"
      >
        {loading ? "…" : "Convert"}
      </button>
      {error && (
        <p className="text-red-500 text-sm self-center">{error}</p>
      )}
    </form>
  );
}
