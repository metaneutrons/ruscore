"use client";

import { useSearchParams } from "next/navigation";
import { Suspense, useEffect, useState } from "react";
import { fetchJob, pdfUrl } from "@/lib/api";
import { Job } from "@/lib/types";
import { StatusBadge } from "@/components/status-badge";

function JobDetail() {
  const searchParams = useSearchParams();
  const id = searchParams.get("id");
  const [job, setJob] = useState<Job | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!id) return;
    let active = true;

    const poll = async () => {
      try {
        const data = await fetchJob(id);
        if (active) setJob(data);
        if (active && (data.status === "queued" || data.status === "processing")) {
          setTimeout(poll, 2000);
        }
      } catch {
        if (active) setError("Failed to load job");
      }
    };

    poll();
    return () => { active = false; };
  }, [id]);

  if (!id) return <p className="text-red-500">No job ID provided.</p>;
  if (error) return <p className="text-red-500">{error}</p>;
  if (!job) return <p className="text-(--color-text-secondary)">Loading…</p>;

  const m = job.metadata;

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-6 flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-bold">{m?.title || "Job"}</h1>
          {m?.composer && <p className="text-(--color-text-secondary)">{m.composer}</p>}
        </div>
        <StatusBadge status={job.status} />
      </div>

      {m?.thumbnail_url && (
        <img src={m.thumbnail_url} alt="" className="mb-6 h-48 rounded-lg object-contain" />
      )}

      <dl className="mb-6 grid grid-cols-2 gap-4 text-sm">
        {m?.arranger && <Field label="Arranger" value={m.arranger} />}
        {m?.instruments?.length ? <Field label="Instruments" value={m.instruments.join(", ")} /> : null}
        {m?.pages ? <Field label="Pages" value={String(m.pages)} /> : null}
        <Field label="Created" value={new Date(job.created_at).toLocaleString()} />
        {m?.description && (
          <div className="col-span-2">
            <dt className="font-medium text-(--color-text-secondary)">Description</dt>
            <dd className="mt-1">{m.description}</dd>
          </div>
        )}
      </dl>

      {job.status === "completed" && (
        <a
          href={pdfUrl(job.id)}
          download
          className="inline-block rounded-lg bg-(--color-accent) px-6 py-2.5 text-sm font-medium text-white hover:bg-(--color-accent-hover)"
        >
          Download PDF
        </a>
      )}

      {job.status === "failed" && job.error && (
        <div className="rounded-lg border border-red-300 bg-red-50 p-4 text-sm text-red-700 dark:border-red-800 dark:bg-red-950 dark:text-red-300">
          {job.error}
        </div>
      )}

      {(job.status === "queued" || job.status === "processing") && (
        <p className="text-sm text-(--color-text-secondary) animate-pulse">
          {job.status === "queued" ? "Waiting in queue…" : "Processing…"}
        </p>
      )}
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="font-medium text-(--color-text-secondary)">{label}</dt>
      <dd className="mt-1">{value}</dd>
    </div>
  );
}

export default function JobDetailPage() {
  return (
    <Suspense fallback={<p className="text-(--color-text-secondary)">Loading…</p>}>
      <JobDetail />
    </Suspense>
  );
}
