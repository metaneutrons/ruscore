export type JobStatus = "queued" | "processing" | "completed" | "failed";

export interface JobMetadata {
  title: string;
  composer: string;
  arranger: string;
  instruments: string[];
  pages: number;
  description: string;
  thumbnail_url: string;
  warnings?: string[];
}

export interface Job {
  id: string;
  url: string;
  url_hash: string;
  status: JobStatus;
  metadata: JobMetadata | null;
  pages: number;
  error: string | null;
  created_at: string;
  updated_at: string;
}

export interface JobListResponse {
  jobs: Job[];
  total: number;
  page: number;
  per_page: number;
}

export interface Suggestion {
  id: string;
  title: string;
  composer: string;
}
