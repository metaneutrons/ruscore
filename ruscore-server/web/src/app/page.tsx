"use client";

import { useState } from "react";
import { UrlInput } from "@/components/url-input";
import { JobList } from "@/components/job-list";

export default function Home() {
  const [refreshKey, setRefreshKey] = useState(0);

  return (
    <div className="space-y-6">
      <UrlInput onSubmitted={() => setRefreshKey((k) => k + 1)} />
      <JobList refreshKey={refreshKey} />
    </div>
  );
}
