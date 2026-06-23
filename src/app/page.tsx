"use client";

import { useEffect, useState } from "react";
import { listDocuments, listTrash, uploadFile, getFlashcards } from "@/lib/api";
import { DocumentInfo } from "@/lib/types";
import DocumentList from "@/components/DocumentList";
import TrashList from "@/components/TrashList";
import UploadZone from "@/components/UploadZone";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { Trash2, ChevronDown, ChevronRight } from "lucide-react";

type DocSummary = Pick<
  DocumentInfo,
  "id" | "filename" | "file_type" | "page_count" | "total_chars"
>;

export default function Home() {
  const [docs, setDocs] = useState<DocSummary[]>([]);
  const [trash, setTrash] = useState<(DocSummary & { deleted_at?: string | null })[]>([]);
  const [cardCounts, setCardCounts] = useState<Record<string, number>>({});
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState("");
  const [trashOpen, setTrashOpen] = useState(false);

  const load = async () => {
    try {
      const [active, trashed] = await Promise.all([
        listDocuments(),
        listTrash(),
      ]);
      setDocs(active);
      setTrash(trashed);

      // Fetch card counts for each document
      const counts: Record<string, number> = {};
      await Promise.all(
        active.map(async (doc) => {
          try {
            const cards = await getFlashcards(doc.id);
            counts[doc.id] = cards.length;
          } catch {
            counts[doc.id] = 0;
          }
        })
      );
      setCardCounts(counts);
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
            <DocumentList docs={docs} cardCounts={cardCounts} onChanged={load} />
          </section>

          {trash.length > 0 && (
            <>
              <Separator />
              <section>
                <Button
                  variant="ghost"
                  onClick={() => setTrashOpen((v) => !v)}
                  className="mb-2 px-2"
                >
                  {trashOpen ? (
                    <ChevronDown className="mr-2 size-4" />
                  ) : (
                    <ChevronRight className="mr-2 size-4" />
                  )}
                  <Trash2 className="mr-2 size-4" />
                  Trash ({trash.length})
                </Button>
                {trashOpen && <TrashList docs={trash} onChanged={load} />}
              </section>
            </>
          )}
        </CardContent>
      </Card>
    </main>
  );
}
