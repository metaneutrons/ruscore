import { JobStatus } from "@/lib/types";

const styles: Record<JobStatus, string> = {
  queued: "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300",
  processing:
    "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300 animate-pulse",
  completed:
    "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
  failed: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
};

export function StatusBadge({ status }: { status: JobStatus }) {
  return (
    <span
      className={`inline-block px-2 py-0.5 rounded-full text-xs font-medium ${styles[status]}`}
    >
      {status}
    </span>
  );
}
