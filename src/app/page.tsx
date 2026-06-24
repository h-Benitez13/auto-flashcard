"use client";

import { useState } from "react";
import { useDocuments, useTrash } from "@/hooks/useDocuments";
import { useMutateUpload } from "@/hooks/useMutations";
import { useUiStore } from "@/lib/store";
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
import { Skeleton } from "@/components/ui/skeleton";
import { Trash2, ChevronDown, ChevronRight } from "lucide-react";

export default function Home() {
  // Server state now managed by TanStack Query
  const { data: docs = [], isLoading: docsLoading } = useDocuments();
  const { data: trash = [] } = useTrash();
  const { mutate: upload, isPending: uploading } = useMutateUpload();

  // UI state managed by Zustand
  const { isTrashOpen, toggleTrash } = useUiStore((state) => ({
    isTrashOpen: state.isTrashOpen,
    toggleTrash: state.toggleTrash,
  }));

  // Local error state for validation before upload
  const [error, setError] = useState("");

  const handleUpload = async (file: File) => {
    setError("");
    upload(file, {
      onError: (e) => {
        setError(e instanceof Error ? e.message : "Upload failed");
      },
    });
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
            {docsLoading ? (
              <div className="space-y-3">
                <Skeleton className="h-20 w-full" />
                <Skeleton className="h-20 w-full" />
                <Skeleton className="h-20 w-full" />
              </div>
            ) : (
              <DocumentList docs={docs} />
            )}
          </section>

          {trash.length > 0 && (
            <>
              <Separator />
              <section>
                <Button
                  variant="ghost"
                  onClick={toggleTrash}
                  className="mb-2 px-2"
                >
                  {isTrashOpen ? (
                    <ChevronDown className="mr-2 size-4" />
                  ) : (
                    <ChevronRight className="mr-2 size-4" />
                  )}
                  <Trash2 className="mr-2 size-4" />
                  Trash ({trash.length})
                </Button>
                {isTrashOpen && <TrashList docs={trash} />}
              </section>
            </>
          )}
        </CardContent>
      </Card>
    </main>
  );
}
