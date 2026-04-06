"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { createJob } from "@/lib/api";

export default function Home() {
  const router = useRouter();
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      const { id } = await createJob(url);
      router.push(`/jobs/detail?id=${id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center">
      <h1 className="mb-2 text-4xl font-bold">Ruscore</h1>
      <p className="mb-8 text-(--color-text-secondary)">Convert MuseScore sheets to PDF</p>
      <form onSubmit={submit} className="w-full max-w-lg">
        <div className="flex gap-2">
          <input
            type="url"
            required
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://musescore.com/..."
            className="flex-1 rounded-lg border border-(--color-border) bg-(--color-bg-secondary) px-4 py-2.5 text-sm outline-none focus:border-(--color-accent) focus:ring-1 focus:ring-(--color-accent)"
          />
          <button
            type="submit"
            disabled={loading}
            className="rounded-lg bg-(--color-accent) px-6 py-2.5 text-sm font-medium text-white hover:bg-(--color-accent-hover) disabled:opacity-50"
          >
            {loading ? "Submitting…" : "Convert"}
          </button>
        </div>
        {error && <p className="mt-3 text-sm text-red-500">{error}</p>}
      </form>
    </div>
  );
}
