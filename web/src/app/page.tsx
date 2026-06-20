"use client";

import { useEffect, useState } from "react";
import { listDocuments, uploadFile } from "@/lib/api";
import { DocumentInfo } from "@/lib/types";
import DocumentList from "@/components/DocumentList";
import UploadZone from "@/components/UploadZone";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";

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
    <main className="mx-auto w-full max-w-3xl p-8">
      <Card>
        <CardHeader>
          <CardTitle className="text-3xl">Flashcards</CardTitle>
          <CardDescription>
            Upload PDFs or Markdown files and generate study cards.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          <UploadZone onUpload={handleUpload} uploading={uploading} />
          {error && (
            <div className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}
          <Separator />
          <section>
            <h2 className="mb-4 text-xl font-semibold">Documents</h2>
            <DocumentList docs={docs} />
          </section>
        </CardContent>
      </Card>
    </main>
  );
}
