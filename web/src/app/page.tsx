"use client";

import { useEffect, useState } from "react";
import { listDocuments, uploadFile } from "@/lib/api";
import { DocumentInfo } from "@/lib/types";
import DocumentList from "@/components/DocumentList";
import UploadZone from "@/components/UploadZone";

export default function Home() {
  const [docs, setDocs] = useState<
    Pick<
      DocumentInfo,
      "id" | "filename" | "file_type" | "page_count" | "total_chars"
    >[]
  >([]);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState("");

  const load = async () => {
    try {
      setDocs(await listDocuments());
    } catch {
      setError("Could not load documents");
    }
  };

  useEffect(() => {
    load();
  }, []);

  const handleUpload = async (file: File) => {
    setUploading(true);
    setError("");
    try {
      await uploadFile(file);
      await load();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Upload failed");
    } finally {
      setUploading(false);
    }
  };

  return (
    <main className="mx-auto max-w-3xl p-8">
      <h1 className="mb-2 text-3xl font-bold">Flashcards</h1>
      <p className="mb-8 text-zinc-600">
        Upload PDFs or Markdown to get started.
      </p>
      <UploadZone onUpload={handleUpload} uploading={uploading} />
      {error && <p className="mt-4 text-red-600">{error}</p>}
      <section className="mt-12">
        <h2 className="mb-4 text-xl font-semibold">Documents</h2>
        <DocumentList docs={docs} />
      </section>
    </main>
  );
}
