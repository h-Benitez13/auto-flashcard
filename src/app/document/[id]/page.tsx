"use client";

import { useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { ArrowLeft, FileText, Loader2, AlertTriangle } from "lucide-react";

import {
  useDocument,
  useFlashcards,
} from "@/hooks/useDocuments";
import { useMutateGenerate } from "@/hooks/useMutations";
import { useGenerationJobPolling } from "@/hooks/useGenerationJob";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import FlashcardList from "@/components/FlashcardList";

type Density = "concise" | "balanced" | "comprehensive";

export default function DocumentPage() {
  const { id } = useParams<{ id: string }>();

  // Server state from TanStack Query
  const {
    data: doc,
    isLoading: docLoading,
    error: docError,
  } = useDocument(id);
  const { data: cards = [], refetch: refetchCards } = useFlashcards(id);

  // Local state for generation
  const [density, setDensity] = useState<Density>("balanced");
  const [jobId, setJobId] = useState<string | null>(null);

  const { mutate: generate, isPending: generating } = useMutateGenerate();

  // Smart polling for generation job
  // - Stops automatically when job completes or fails
  // - Adaptive backoff: 1s -> 2s -> 5s -> 10s
  // - Max 1 hour polling duration
  // - Auto-refetches flashcards on completion
  const { data: job } = useGenerationJobPolling({
    docId: id,
    jobId: jobId || "",
    onCompleted: () => {
      refetchCards();
    },
  });

  const handleGenerate = () => {
    generate(
      { docId: id, density },
      {
        onSuccess: (result) => {
          setJobId(result.jobId);
        },
      }
    );
  };

  // Loading state
  if (docLoading) {
    return (
      <main className="mx-auto w-full max-w-4xl p-8 space-y-6">
        <Skeleton className="h-10 w-32" />
        <Skeleton className="h-32 w-full" />
        <Skeleton className="h-64 w-full" />
      </main>
    );
  }

  // Deleted document state (404)
  if (docError && "status" in docError && docError.status === 404) {
    return (
      <main className="mx-auto flex min-h-screen w-full max-w-4xl flex-col items-center justify-center p-8 text-center">
        <div className="mb-4 rounded-full bg-amber-100 p-4 dark:bg-amber-900">
          <AlertTriangle className="size-8 text-amber-600 dark:text-amber-400" />
        </div>
        <h1 className="mb-2 text-2xl font-semibold">Document deleted</h1>
        <p className="mb-6 max-w-md text-muted-foreground">
          This document has been moved to trash or permanently removed.
        </p>
        <Button asChild>
          <Link href="/">Back to documents</Link>
        </Button>
      </main>
    );
  }

  // Generic error state
  if (docError) {
    return (
      <p className="p-8 text-destructive">
        {docError instanceof Error
          ? docError.message
          : "Could not load document"}
      </p>
    );
  }

  if (!doc) return null;

  const isGenerating = job?.status === "generating";
  const progressPercent =
    job && job.total > 0 ? Math.round((job.progress / job.total) * 100) : 0;

  return (
    <main className="mx-auto w-full max-w-4xl p-8">
      <Button variant="ghost" size="sm" asChild className="mb-4">
        <Link href="/">
          <ArrowLeft className="mr-2 size-4" />
          Back
        </Link>
      </Button>

      <Card className="mb-8">
        <CardHeader className="flex flex-row items-center gap-4">
          <div className="rounded-full bg-secondary p-3">
            <FileText className="size-6 text-secondary-foreground" />
          </div>
          <div>
            <CardTitle className="text-2xl">{doc.filename}</CardTitle>
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Badge variant="secondary" className="uppercase">
                {doc.file_type}
              </Badge>
              <span>
                {doc.page_count} pages · {doc.total_chars.toLocaleString()} chars
              </span>
            </div>
          </div>
        </CardHeader>
      </Card>

      <Card className="mb-8">
        <CardHeader>
          <CardTitle>Generate flashcards</CardTitle>
          <CardDescription>
            Choose density and generate cards from this document.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
            <label htmlFor="density" className="text-sm font-medium">
              Density
            </label>
            <select
              id="density"
              value={density}
              onChange={(e) => setDensity(e.target.value as Density)}
              className="h-10 rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
            >
              <option value="concise">Concise</option>
              <option value="balanced">Balanced</option>
              <option value="comprehensive">Comprehensive</option>
            </select>
          </div>

          <Button onClick={handleGenerate} disabled={generating || isGenerating}>
            {generating || isGenerating ? (
              <>
                <Loader2 className="mr-2 size-4 animate-spin" />
                Generating…
              </>
            ) : (
              "Generate flashcards"
            )}
          </Button>

          {isGenerating && (
            <div className="space-y-1">
              <div className="flex justify-between text-sm">
                <span>{job?.status_message || "Generating…"}</span>
                <span>
                  {job?.progress} / {job?.total}
                </span>
              </div>
              <div className="h-2 w-full rounded-full bg-secondary">
                <div
                  className="h-2 rounded-full bg-primary transition-all"
                  style={{ width: `${progressPercent}%` }}
                />
              </div>
            </div>
          )}

          {job?.status === "failed" && (
            <p className="text-sm text-destructive">
              Generation failed: {job.error_message}
            </p>
          )}

          {job?.status === "completed_fallback" && (
            <p className="text-sm text-amber-600 dark:text-amber-400">
              {job.error_message ||
                "Some cards used rule-based fallback (LLM rate-limited). You can regenerate later for higher-quality cards."}
            </p>
          )}
        </CardContent>
      </Card>

      {cards.length > 0 && (
        <>
          <Separator className="my-8" />
          <section>
            <h2 className="mb-4 text-xl font-semibold">Flashcards</h2>
            <FlashcardList cards={cards} />
          </section>
        </>
      )}

      <details className="mt-8">
        <summary className="cursor-pointer text-sm font-medium text-muted-foreground">
          Extracted pages
        </summary>
        <div className="mt-4 space-y-4">
          {doc.pages.map((page) => (
            <Card key={page.page_num}>
              <CardHeader className="pb-3">
                <CardDescription className="font-semibold uppercase">
                  Page {page.page_num}
                </CardDescription>
              </CardHeader>
              <Separator />
              <CardContent className="pt-4">
                <pre className="whitespace-pre-wrap font-sans text-sm leading-relaxed">
                  {page.text}
                </pre>
              </CardContent>
            </Card>
          ))}
        </div>
      </details>
    </main>
  );
}
