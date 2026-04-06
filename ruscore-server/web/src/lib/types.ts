export type JobStatus = "queued" | "processing" | "completed" | "failed";

export interface JobMetadata {
  title: string;
  composer: string;
  arranger: string;
  instruments: string[];
  pages: number;
  description: string;
  thumbnail_url: string;
}

export interface Job {
  id: string;
  url: string;
  status: JobStatus;
  metadata: JobMetadata | null;
  created_at: string;
  error: string | null;
}

export interface JobListResponse {
  jobs: Job[];
  total: number;
  page: number;
  per_page: number;
}
