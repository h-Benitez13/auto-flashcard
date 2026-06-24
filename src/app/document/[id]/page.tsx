"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { ArrowLeft, FileText, Loader2 } from "lucide-react";

import { getDocument, generateFlashcards, getJob, getFlashcards } from "@/lib/api";
import { DocumentInfo, Flashcard, GenerationJob } from "@/lib/types";
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
import FlashcardList from "@/components/FlashcardList";

type Density = "concise" | "balanced" | "comprehensive";

export default function DocumentPage() {
  const { id } = useParams<{ id: string }>();
  const [doc, setDoc] = useState<DocumentInfo | null>(null);
  const [cards, setCards] = useState<Flashcard[]>([]);
  const [job, setJob] = useState<GenerationJob | null>(null);
  const [density, setDensity] = useState<Density>("balanced");
  const [loading, setLoading] = useState(true);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    Promise.all([getDocument(id), getFlashcards(id)])
      .then(([d, c]) => {
        setDoc(d);
        setCards(c);
      })
      .catch(() => setError("Could not load document"))
      .finally(() => setLoading(false));
  }, [id]);

  const isDone = job && (job.status === "completed" || job.status === "completed_fallback");

  useEffect(() => {
    if (!job || isDone || job.status === "failed") return;

    const interval = setInterval(async () => {
      try {
        const updated = await getJob(job.id);
        setJob(updated);
        if (updated.status === "completed" || updated.status === "completed_fallback") {
          const fresh = await getFlashcards(id);
          setCards(fresh);
        }
      } catch {
        // ignore polling errors
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [job, id, isDone]);

  const handleGenerate = async () => {
    setGenerating(true);
    setError("");
    try {
      const { job_id } = await generateFlashcards(id, { density });
      const started = await getJob(job_id);
      setJob(started);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Generation failed");
    } finally {
      setGenerating(false);
    }
  };

  if (loading) return <p className="p-8">Loading…</p>;
  if (error) return <p className="p-8 text-destructive">{error}</p>;
  if (!doc) return null;

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

          <Button onClick={handleGenerate} disabled={generating || !!job && job.status === "generating"}>
            {generating || (job && job.status === "generating") ? (
              <>
                <Loader2 className="mr-2 size-4 animate-spin" />
                Generating…
              </>
            ) : (
              "Generate flashcards"
            )}
          </Button>

          {job && job.status === "generating" && (
            <div className="space-y-1">
              <div className="flex justify-between text-sm">
                <span>{job.status_message || "Generating…"}</span>
                <span>
                  {job.progress} / {job.total}
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
            <FlashcardList key={cards.map((c) => c.id).join(",")} cards={cards} />
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
