"use client";

import { useCallback, useEffect, useState } from "react";
import { Flashcard } from "@/lib/types";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Shuffle, LayoutGrid, BookOpen, ChevronLeft, ChevronRight } from "lucide-react";

interface Props {
  cards: Flashcard[];
}

type ViewMode = "study" | "grid";
type GridColumns = 1 | 2 | 3;

function providerLabel(provider?: Flashcard["provider"]) {
  if (provider === "rule-based") return "Rule-based";
  if (provider === "llm") return "LLM";
  return undefined;
}

function ProviderBadge({ provider }: { provider?: Flashcard["provider"] }) {
  const label = providerLabel(provider);
  if (!label) return null;
  return (
    <Badge
      variant={provider === "rule-based" ? "secondary" : "default"}
      className="text-xs"
    >
      {label}
    </Badge>
  );
}

export default function FlashcardList({ cards }: Props) {
  const [mode, setMode] = useState<ViewMode>("study");
  const [columns, setColumns] = useState<GridColumns>(1);

  if (cards.length === 0) {
    return <p className="text-sm text-muted-foreground">No flashcards yet.</p>;
  }

  return (
    <div className="space-y-6">
      {/* View mode toggle */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-2 rounded-md border bg-background p-1">
          <button
            onClick={() => setMode("study")}
            className={`flex items-center gap-2 rounded px-3 py-1.5 text-xs font-medium transition-colors ${
              mode === "study"
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-muted"
            }`}
          >
            <BookOpen className="size-3.5" />
            Study
          </button>
          <button
            onClick={() => setMode("grid")}
            className={`flex items-center gap-2 rounded px-3 py-1.5 text-xs font-medium transition-colors ${
              mode === "grid"
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-muted"
            }`}
          >
            <LayoutGrid className="size-3.5" />
            Grid
          </button>
        </div>

        {mode === "grid" && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">Columns</span>
            <div className="flex items-center rounded-md border bg-background p-1">
              {[1, 2, 3].map((col) => (
                <button
                  key={col}
                  onClick={() => setColumns(col as GridColumns)}
                  className={`rounded px-2.5 py-1 text-xs font-medium transition-colors ${
                    columns === col
                      ? "bg-primary text-primary-foreground"
                      : "text-muted-foreground hover:bg-muted"
                  }`}
                  aria-label={`${col} column view`}
                >
                  {col}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      {mode === "study" ? (
        <StudyView cards={cards} />
      ) : (
        <GridView cards={cards} columns={columns} />
      )}
    </div>
  );
}

function StudyView({ cards }: { cards: Flashcard[] }) {
  const [order, setOrder] = useState<number[]>(() =>
    Array.from({ length: cards.length }, (_, i) => i)
  );
  const [currentIdx, setCurrentIdx] = useState(0);
  const [isFlipped, setIsFlipped] = useState(false);

  const shuffle = () => {
    const next = [...order];
    for (let i = next.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [next[i], next[j]] = [next[j], next[i]];
    }
    setOrder(next);
    setCurrentIdx(0);
    setIsFlipped(false);
  };

  const goNext = useCallback(() => {
    if (currentIdx < cards.length - 1) {
      setCurrentIdx((idx) => idx + 1);
      setIsFlipped(false);
    }
  }, [currentIdx, cards.length]);

  const goPrev = useCallback(() => {
    if (currentIdx > 0) {
      setCurrentIdx((idx) => idx - 1);
      setIsFlipped(false);
    }
  }, [currentIdx]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Space") {
        e.preventDefault();
        setIsFlipped((prev) => !prev);
      } else if (e.code === "ArrowRight") {
        goNext();
      } else if (e.code === "ArrowLeft") {
        goPrev();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [goNext, goPrev]);

  const cardIdx = order[currentIdx];
  const card = cardIdx !== undefined ? cards[cardIdx] : undefined;
  if (!card) {
    return <p className="text-sm text-muted-foreground">No flashcards yet.</p>;
  }

  const rawPreview = card.source_ref?.preview ?? "";
  const preview =
    rawPreview.length > 200 ? rawPreview.substring(0, 200) + "..." : rawPreview;

  return (
    <div className="flex flex-col items-center justify-center gap-8">
      <div className="flex w-full max-w-2xl items-center justify-between">
        <Button variant="outline" size="sm" onClick={shuffle}>
          <Shuffle className="mr-2 size-4" />
          Shuffle
        </Button>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <ProviderBadge provider={card.provider} />
          <span>
            {currentIdx + 1} of {order.length}
          </span>
        </div>
      </div>

      <Card
        onClick={() => setIsFlipped(!isFlipped)}
        className="w-full max-w-2xl cursor-pointer transition-all hover:shadow-lg active:scale-98"
      >
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <Badge variant="outline" className="uppercase">
              {card.card_type}
            </Badge>
            <span className="text-xs text-muted-foreground">
              Page {card.source_ref.page_start}
              {card.source_ref.page_end !== card.source_ref.page_start
                ? `–${card.source_ref.page_end}`
                : ""}
            </span>
          </div>
        </CardHeader>
        <CardContent className="flex min-h-64 flex-col justify-between">
          <div>
            {isFlipped ? (
              <>
                <div className="text-sm text-muted-foreground mb-4">Answer</div>
                <div className="whitespace-pre-wrap text-xl leading-relaxed">
                  {card.answer}
                </div>

                <div className="mt-6 border-t bg-slate-50 dark:bg-slate-900 -mx-6 px-6 py-4">
                  <div className="text-xs font-medium text-muted-foreground mb-2">
                    Source
                  </div>
                  <div className="font-mono text-xs text-muted-foreground leading-relaxed">
                    {preview}
                  </div>
                </div>
              </>
            ) : (
              <>
                <div className="text-sm text-muted-foreground mb-4">Question</div>
                <div className="text-2xl font-semibold leading-relaxed">
                  {card.question}
                </div>
              </>
            )}
          </div>
        </CardContent>
      </Card>

      <div className="flex gap-2">
        <Button variant="outline" size="lg" onClick={goPrev} disabled={currentIdx === 0}>
          <ChevronLeft className="mr-2 size-5" />
          Previous
        </Button>
        <Button
          variant="outline"
          size="lg"
          onClick={goNext}
          disabled={currentIdx === order.length - 1}
        >
          Next
          <ChevronRight className="ml-2 size-5" />
        </Button>
      </div>

      <div className="text-xs text-muted-foreground text-center">
        <kbd className="rounded bg-slate-100 dark:bg-slate-800 px-2 py-1 text-xs">Space</kbd> to
        flip •{" "}
        <kbd className="rounded bg-slate-100 dark:bg-slate-800 px-2 py-1 text-xs">←</kbd> /{" "}
        <kbd className="rounded bg-slate-100 dark:bg-slate-800 px-2 py-1 text-xs">→</kbd> to
        navigate
      </div>
    </div>
  );
}

function GridView({ cards, columns }: { cards: Flashcard[]; columns: GridColumns }) {
  const [flipped, setFlipped] = useState<Record<string, boolean>>({});

  const gridClass =
    columns === 3
      ? "grid-cols-1 md:grid-cols-3"
      : columns === 2
        ? "grid-cols-1 md:grid-cols-2"
        : "grid-cols-1";

  const toggleFlip = (cardId: string) => {
    setFlipped((prev) => ({ ...prev, [cardId]: !prev[cardId] }));
  };

  return (
    <div className="space-y-6">
      <div className={`grid ${gridClass} gap-4`}>
        {cards.map((card) => {
          const isFlipped = flipped[card.id] ?? false;
          const rawPreview = card.source_ref?.preview ?? "";
          const preview =
            rawPreview.length > 200 ? rawPreview.substring(0, 200) + "..." : rawPreview;

          return (
            <Card
              key={card.id}
              onClick={() => toggleFlip(card.id)}
              className="flex h-full cursor-pointer flex-col transition-all hover:shadow-lg active:scale-[0.99]"
            >
              <CardHeader className="pb-3">
                <div className="flex items-center justify-between gap-2">
                  <Badge variant="outline" className="uppercase">
                    {card.card_type}
                  </Badge>
                  <div className="flex items-center gap-2">
                    <ProviderBadge provider={card.provider} />
                    <span className="text-xs text-muted-foreground">
                      Page {card.source_ref.page_start}
                      {card.source_ref.page_end !== card.source_ref.page_start
                        ? `–${card.source_ref.page_end}`
                        : ""}
                    </span>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="flex flex-1 flex-col justify-between">
                {isFlipped ? (
                  <>
                    <div className="text-sm text-muted-foreground mb-2">Answer</div>
                    <div className="whitespace-pre-wrap text-lg leading-relaxed">
                      {card.answer}
                    </div>

                    <div className="mt-6 border-t bg-slate-50 dark:bg-slate-900 -mx-6 px-6 py-4">
                      <div className="text-xs font-medium text-muted-foreground mb-2">
                        Source
                      </div>
                      <div className="font-mono text-xs text-muted-foreground leading-relaxed">
                        {preview}
                      </div>
                    </div>
                  </>
                ) : (
                  <>
                    <div className="text-sm text-muted-foreground mb-2">Question</div>
                    <div className="text-xl font-semibold leading-relaxed">
                      {card.question}
                    </div>
                    <div className="mt-auto pt-6 text-xs text-muted-foreground">
                      Click to flip
                    </div>
                  </>
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>

      <div className="text-xs text-muted-foreground text-center">
        Click any card to flip
      </div>
    </div>
  );
}
