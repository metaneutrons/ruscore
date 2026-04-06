import { Job, JobListResponse, JobStatus } from "./types";

const API = "/api/v1";

export async function createJob(url: string): Promise<{ id: string; status: string; conflict: boolean }> {
  const res = await fetch(`${API}/jobs`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url }),
  });
  if (res.status === 409) {
    const data = await res.json();
    return { ...data, conflict: true };
  }
  if (!res.ok) throw new Error(`Failed to create job: ${res.status}`);
  const data = await res.json();
  return { ...data, conflict: false };
}

export async function fetchJobs(page: number, perPage: number, status?: JobStatus, sort?: string, order?: string, q?: string): Promise<JobListResponse> {
  const params = new URLSearchParams({ page: String(page), per_page: String(perPage) });
  if (status) params.set("status", status);
  if (sort) params.set("sort", sort);
  if (order) params.set("order", order);
  if (q) params.set("q", q);
  const res = await fetch(`${API}/jobs?${params}`);
  if (!res.ok) throw new Error(`Failed to fetch jobs: ${res.status}`);
  return res.json();
}

export async function fetchJob(id: string): Promise<Job> {
  const res = await fetch(`${API}/jobs/${id}`);
  if (!res.ok) throw new Error(`Failed to fetch job: ${res.status}`);
  return res.json();
}

export function pdfUrl(id: string): string {
  return `${API}/jobs/${id}/pdf`;
}

export interface Suggestion {
  id: string;
  title: string;
  composer: string;
}

export async function fetchSuggestions(q: string, limit = 5): Promise<Suggestion[]> {
  if (!q.trim()) return [];
  const params = new URLSearchParams({ q, limit: String(limit) });
  const res = await fetch(`${API}/jobs/suggest?${params}`);
  if (!res.ok) return [];
  return res.json();
}
