"use client";

import { Suspense, useCallback, useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import Link from "next/link";
import { fetchJob, pdfUrl } from "@/lib/api";
import { Job } from "@/lib/types";
import { StatusBadge } from "@/components/status-badge";

function DetailContent() {
  const params = useSearchParams();
  const id = params.get("id");
  const [job, setJob] = useState<Job | null>(null);
  const [error, setError] = useState("");

  const load = useCallback(async () => {
    if (!id) return;
    try {
      setJob(await fetchJob(id));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load job");
    }
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  // Auto-refresh while pending
  useEffect(() => {
    if (!job || (job.status !== "queued" && job.status !== "processing")) return;
    const timer = setTimeout(load, 2000);
    return () => clearTimeout(timer);
  }, [job, load]);

  if (!id) return <p>No job ID provided.</p>;
  if (error) return <p className="text-red-500">{error}</p>;
  if (!job) return <p className="text-(--color-text-secondary)">Loading…</p>;

  const m = job.metadata;

  return (
    <div className="space-y-6">
      <Link
        href="/"
        className="text-(--color-accent) hover:underline text-sm"
      >
        ← Back to jobs
      </Link>

      <div className="flex flex-col sm:flex-row gap-6">
        {/* Thumbnail */}
        {m?.thumbnail_url && (
          <img
            src={m.thumbnail_url}
            alt=""
            className="w-48 h-auto rounded-lg border border-(--color-border) object-contain"
          />
        )}

        {/* Header */}
        <div className="space-y-2">
          <h1 className="text-2xl font-bold">
            {m?.title || job.url}
          </h1>
          {m?.composer && (
            <p className="text-lg text-(--color-text-secondary)">
              {m.composer}
            </p>
          )}
          <StatusBadge status={job.status} />
        </div>
      </div>

      {/* Error */}
      {job.error && (
        <div className="p-4 rounded-lg bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 border border-red-300 dark:border-red-800">
          {job.error}
        </div>
      )}

      {/* Metadata grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 text-sm">
        {m?.arranger && (
          <div>
            <span className="font-medium">Arranger:</span>{" "}
            <span className="text-(--color-text-secondary)">{m.arranger}</span>
          </div>
        )}
        {m?.instruments && m.instruments.length > 0 && (
          <div>
            <span className="font-medium">Instruments:</span>{" "}
            <span className="text-(--color-text-secondary)">
              {m.instruments.join(", ")}
            </span>
          </div>
        )}
        <div>
          <span className="font-medium">Pages:</span>{" "}
          <span className="text-(--color-text-secondary)">
            {job.pages || m?.pages || "—"}
          </span>
        </div>
        <div>
          <span className="font-medium">URL:</span>{" "}
          <a
            href={job.url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-(--color-accent) hover:underline break-all"
          >
            {job.url}
          </a>
        </div>
        <div>
          <span className="font-medium">Created:</span>{" "}
          <span className="text-(--color-text-secondary)">
            {new Date(job.created_at).toLocaleString()}
          </span>
        </div>
        {m?.description && (
          <div className="sm:col-span-2">
            <span className="font-medium">Description:</span>{" "}
            <span className="text-(--color-text-secondary)">
              {m.description}
            </span>
          </div>
        )}
      </div>

      {/* Download */}
      {job.status === "completed" && (
        <a
          href={pdfUrl(job.id)}
          className="inline-block px-6 py-3 rounded-lg bg-(--color-accent) text-white font-medium hover:bg-(--color-accent-hover) transition-colors"
        >
          ⬇ Download PDF
        </a>
      )}
    </div>
  );
}

export default function DetailPage() {
  return (
    <Suspense
      fallback={
        <p className="text-(--color-text-secondary)">Loading…</p>
      }
    >
      <DetailContent />
    </Suspense>
  );
}
