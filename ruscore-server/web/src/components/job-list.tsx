"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { deleteJob, deleteJobs, fetchJobs, retryJob } from "@/lib/api";
import { Job, JobListResponse, JobStatus } from "@/lib/types";
import { StatusBadge } from "./status-badge";
import { SearchBar } from "./search-bar";

type SortField = "title" | "composer" | "pages" | "status" | "created_at";

export function JobList({ refreshKey }: { refreshKey: number }) {
  const [data, setData] = useState<JobListResponse | null>(null);
  const [page, setPage] = useState(1);
  const [perPage] = useState(20);
  const [status, setStatus] = useState<JobStatus | "">("");
  const [sort, setSort] = useState<SortField>("created_at");
  const [order, setOrder] = useState<"asc" | "desc">("desc");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [localRefresh, setLocalRefresh] = useState(0);
  const pollRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const refresh = () => setLocalRefresh((n) => n + 1);

  const load = useCallback(async () => {
    try {
      const result = await fetchJobs(
        page,
        perPage,
        status || undefined,
        sort,
        order,
        query || undefined,
      );
      setData(result);
    } catch {
      /* ignore */
    }
  }, [page, perPage, status, sort, order, query]);

  // Load on param change or refreshKey
  useEffect(() => {
    load();
  }, [load, refreshKey, localRefresh]);

  // Auto-refresh polling
  useEffect(() => {
    clearTimeout(pollRef.current);
    if (
      data?.jobs.some(
        (j) => j.status === "queued" || j.status === "processing",
      )
    ) {
      pollRef.current = setTimeout(load, 5000);
    }
    return () => clearTimeout(pollRef.current);
  }, [data, load]);

  const toggleSort = (field: SortField) => {
    if (sort === field) {
      setOrder(order === "asc" ? "desc" : "asc");
    } else {
      setSort(field);
      setOrder("asc");
    }
    setPage(1);
  };

  const sortIcon = (field: SortField) =>
    sort === field ? (order === "asc" ? " ↑" : " ↓") : "";

  const toggleSelect = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  };

  const toggleAll = () => {
    if (!data) return;
    if (selected.size === data.jobs.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(data.jobs.map((j) => j.id)));
    }
  };

  const handleBulkDelete = async () => {
    if (selected.size === 0) return;
    if (!confirm(`Delete ${selected.size} job(s)?`)) return;
    await deleteJobs([...selected]);
    setSelected(new Set());
    load();
  };

  const totalPages = data ? Math.max(1, Math.ceil(data.total / perPage)) : 1;

  const jobTitle = (j: Job) =>
    j.metadata?.title || j.url.replace(/^https?:\/\//, "").slice(0, 50);

  return (
    <div className="space-y-4">
      {/* Controls */}
      <div className="flex flex-col sm:flex-row gap-3">
        <div className="flex-1">
          <SearchBar
            onSearch={(q) => {
              setQuery(q);
              setPage(1);
            }}
          />
        </div>
        <select
          value={status}
          onChange={(e) => {
            setStatus(e.target.value as JobStatus | "");
            setPage(1);
          }}
          className="px-3 py-2 rounded-lg border border-(--color-border) bg-(--color-bg) text-(--color-text)"
        >
          <option value="">All statuses</option>
          <option value="queued">Queued</option>
          <option value="processing">Processing</option>
          <option value="completed">Completed</option>
          <option value="failed">Failed</option>
        </select>
        {selected.size > 0 && (
          <button
            onClick={handleBulkDelete}
            className="px-4 py-2 rounded-lg bg-red-600 text-white font-medium hover:bg-red-700 transition-colors"
          >
            🗑 Delete ({selected.size})
          </button>
        )}
      </div>

      {/* Table */}
      <div className="overflow-x-auto rounded-lg border border-(--color-border)">
        <table className="w-full text-sm">
          <thead className="bg-(--color-bg-secondary) text-(--color-text-secondary)">
            <tr>
              <th className="p-3 w-8">
                <input
                  type="checkbox"
                  checked={
                    !!data?.jobs.length && selected.size === data.jobs.length
                  }
                  onChange={toggleAll}
                />
              </th>
              <th className="p-3 w-12"></th>
              <th
                className="p-3 text-left cursor-pointer select-none"
                onClick={() => toggleSort("title")}
              >
                Title{sortIcon("title")}
              </th>
              <th
                className="p-3 text-left cursor-pointer select-none hidden md:table-cell"
                onClick={() => toggleSort("composer")}
              >
                Composer{sortIcon("composer")}
              </th>
              <th
                className="p-3 text-center cursor-pointer select-none hidden sm:table-cell"
                onClick={() => toggleSort("pages")}
              >
                Pages{sortIcon("pages")}
              </th>
              <th
                className="p-3 text-center cursor-pointer select-none"
                onClick={() => toggleSort("status")}
              >
                Status{sortIcon("status")}
              </th>
              <th
                className="p-3 text-left cursor-pointer select-none hidden lg:table-cell"
                onClick={() => toggleSort("created_at")}
              >
                Created{sortIcon("created_at")}
              </th>
              <th className="p-3 text-center w-20"></th>
            </tr>
          </thead>
          <tbody>
            {data?.jobs.map((j) => (
              <tr
                key={j.id}
                className="border-t border-(--color-border) hover:bg-(--color-bg-secondary) transition-colors"
              >
                <td className="p-3">
                  <input
                    type="checkbox"
                    checked={selected.has(j.id)}
                    onChange={() => toggleSelect(j.id)}
                  />
                </td>
                <td className="p-3">
                  {j.metadata?.thumbnail_url && (
                    <img
                      src={j.metadata.thumbnail_url}
                      alt=""
                      className="w-10 h-10 rounded object-cover"
                    />
                  )}
                </td>
                <td className="p-3">
                  <Link
                    href={`/jobs/detail?id=${j.id}`}
                    className="text-(--color-accent) hover:underline font-medium"
                  >
                    {jobTitle(j)}
                  </Link>
                </td>
                <td className="p-3 text-(--color-text-secondary) hidden md:table-cell">
                  {j.metadata?.composer || "—"}
                </td>
                <td className="p-3 text-center hidden sm:table-cell">
                  {j.pages || j.metadata?.pages || "—"}
                </td>
                <td className="p-3 text-center">
                  <StatusBadge status={j.status} />
                </td>
                <td className="p-3 text-(--color-text-secondary) hidden lg:table-cell">
                  {new Date(j.created_at).toLocaleDateString()}
                </td>
                <td className="p-3 text-center">
                  <div className="flex items-center justify-center gap-1">
                    {j.status === "failed" && (
                      <button
                        title="Retry"
                        onClick={async (e) => {
                          e.stopPropagation();
                          try { await retryJob(j.id); refresh(); } catch {}
                        }}
                        className="p-1 rounded hover:bg-(--color-bg-secondary) text-(--color-text-secondary) hover:text-blue-500"
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                          <path fillRule="evenodd" d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z" clipRule="evenodd" />
                        </svg>
                      </button>
                    )}
                    {j.status !== "processing" && (
                      <button
                        title="Delete"
                        onClick={async (e) => {
                          e.stopPropagation();
                          if (!confirm("Delete this job?")) return;
                          try { await deleteJob(j.id); refresh(); } catch {}
                        }}
                        className="p-1 rounded hover:bg-(--color-bg-secondary) text-(--color-text-secondary) hover:text-red-500"
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                          <path fillRule="evenodd" d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z" clipRule="evenodd" />
                        </svg>
                      </button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
            {data && data.jobs.length === 0 && (
              <tr>
                <td
                  colSpan={8}
                  className="p-8 text-center text-(--color-text-secondary)"
                >
                  No jobs found
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      {data && data.total > 0 && (
        <div className="flex items-center justify-between text-sm text-(--color-text-secondary)">
          <span>
            Page {page} of {totalPages} ({data.total} total)
          </span>
          <div className="flex gap-2">
            <button
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              disabled={page <= 1}
              className="px-3 py-1 rounded border border-(--color-border) hover:bg-(--color-bg-tertiary) disabled:opacity-40 transition-colors"
            >
              ← Previous
            </button>
            <button
              onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
              disabled={page >= totalPages}
              className="px-3 py-1 rounded border border-(--color-border) hover:bg-(--color-bg-tertiary) disabled:opacity-40 transition-colors"
            >
              Next →
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
