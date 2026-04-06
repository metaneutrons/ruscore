"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { fetchJobs } from "@/lib/api";
import { Job, JobStatus } from "@/lib/types";
import { StatusBadge } from "@/components/status-badge";

const PER_PAGE = 20;
const STATUSES: (JobStatus | "")[] = ["", "queued", "processing", "completed", "failed"];

export default function JobsPage() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [status, setStatus] = useState<JobStatus | "">("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    fetchJobs(page, PER_PAGE, status || undefined)
      .then((data) => { setJobs(data.jobs); setTotal(data.total); })
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [page, status]);

  const totalPages = Math.max(1, Math.ceil(total / PER_PAGE));

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold">Jobs</h1>
        <select
          value={status}
          onChange={(e) => { setStatus(e.target.value as JobStatus | ""); setPage(1); }}
          className="rounded-lg border border-(--color-border) bg-(--color-bg-secondary) px-3 py-1.5 text-sm"
        >
          {STATUSES.map((s) => (
            <option key={s} value={s}>{s || "All statuses"}</option>
          ))}
        </select>
      </div>

      {loading ? (
        <p className="text-(--color-text-secondary)">Loading…</p>
      ) : jobs.length === 0 ? (
        <p className="text-(--color-text-secondary)">No jobs found.</p>
      ) : (
        <div className="overflow-hidden rounded-lg border border-(--color-border)">
          <table className="w-full text-left text-sm">
            <thead className="bg-(--color-bg-secondary) text-xs uppercase text-(--color-text-secondary)">
              <tr>
                <th className="px-4 py-3">Score</th>
                <th className="px-4 py-3">Composer</th>
                <th className="hidden px-4 py-3 sm:table-cell">Instruments</th>
                <th className="hidden px-4 py-3 sm:table-cell">Pages</th>
                <th className="px-4 py-3">Status</th>
                <th className="hidden px-4 py-3 sm:table-cell">Created</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-(--color-border)">
              {jobs.map((job) => (
                <tr key={job.id} className="hover:bg-(--color-bg-secondary)">
                  <td className="px-4 py-3">
                    <Link href={`/jobs/detail?id=${job.id}`} className="flex items-center gap-3 hover:text-(--color-accent)">
                      {job.metadata?.thumbnail_url && (
                        <img src={job.metadata.thumbnail_url} alt="" className="h-10 w-8 rounded object-cover" />
                      )}
                      <span className="font-medium">{job.metadata?.title || "—"}</span>
                    </Link>
                  </td>
                  <td className="px-4 py-3 text-(--color-text-secondary)">{job.metadata?.composer || "—"}</td>
                  <td className="hidden px-4 py-3 text-(--color-text-secondary) sm:table-cell">
                    {job.metadata?.instruments?.join(", ") || "—"}
                  </td>
                  <td className="hidden px-4 py-3 text-(--color-text-secondary) sm:table-cell">{job.metadata?.pages ?? "—"}</td>
                  <td className="px-4 py-3"><StatusBadge status={job.status} /></td>
                  <td className="hidden px-4 py-3 text-(--color-text-secondary) sm:table-cell">
                    {new Date(job.created_at).toLocaleDateString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {totalPages > 1 && (
        <div className="mt-4 flex items-center justify-center gap-2">
          <button
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={page === 1}
            className="rounded-md border border-(--color-border) px-3 py-1 text-sm disabled:opacity-40"
          >
            Previous
          </button>
          <span className="text-sm text-(--color-text-secondary)">
            {page} / {totalPages}
          </span>
          <button
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={page === totalPages}
            className="rounded-md border border-(--color-border) px-3 py-1 text-sm disabled:opacity-40"
          >
            Next
          </button>
        </div>
      )}
    </div>
  );
}
