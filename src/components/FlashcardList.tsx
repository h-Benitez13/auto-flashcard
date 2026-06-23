"use client";

import { useEffect, useState } from "react";
import { Flashcard } from "@/lib/types";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ChevronLeft, ChevronRight, Shuffle } from "lucide-react";

interface Props {
  cards: Flashcard[];
}

export default function FlashcardList({ cards }: Props) {
  const [order, setOrder] = useState<number[]>(() =>
    Array.from({ length: cards.length }, (_, i) => i)
  );
  const [currentIdx, setCurrentIdx] = useState(0);
  const [isFlipped, setIsFlipped] = useState(false);

  // Sync order when cards change
  useEffect(() => {
    setOrder(Array.from({ length: cards.length }, (_, i) => i));
    setCurrentIdx(0);
    setIsFlipped(false);
  }, [cards.length]);

  if (cards.length === 0) {
    return <p className="text-sm text-muted-foreground">No flashcards yet.</p>;
  }

  // Guard against empty/missing order during transitions
  const cardIdx = order[currentIdx];
  const card = cardIdx !== undefined ? cards[cardIdx] : undefined;
  if (!card) {
    return <p className="text-sm text-muted-foreground">No flashcards yet.</p>;
  }

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

  const goNext = () => {
    if (currentIdx < order.length - 1) {
      setCurrentIdx(currentIdx + 1);
      setIsFlipped(false);
    }
  };

  const goPrev = () => {
    if (currentIdx > 0) {
      setCurrentIdx(currentIdx - 1);
      setIsFlipped(false);
    }
  };

  // Keyboard navigation
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
  }, [currentIdx, order.length]);

  const progress = currentIdx + 1;
  const total = order.length;

  // Truncate preview to ~200 chars
  const rawPreview = card.source_ref?.preview ?? "";
  const preview =
    rawPreview.length > 200
      ? rawPreview.substring(0, 200) + "..."
      : rawPreview;

  return (
    <div className="flex flex-col items-center justify-center gap-8">
      {/* Header with controls */}
      <div className="flex w-full max-w-2xl items-center justify-between">
        <Button variant="outline" size="sm" onClick={shuffle}>
          <Shuffle className="mr-2 size-4" />
          Shuffle
        </Button>
        <div className="text-sm text-muted-foreground">
          {progress} of {total}
        </div>
      </div>

      {/* Centered card */}
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

                {/* Source snippet footer - only visible on answer side */}
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

      {/* Navigation */}
      <div className="flex gap-2">
        <Button
          variant="outline"
          size="lg"
          onClick={goPrev}
          disabled={currentIdx === 0}
        >
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

      {/* Keyboard hints */}
      <div className="text-xs text-muted-foreground text-center">
        <kbd className="rounded bg-slate-100 dark:bg-slate-800 px-2 py-1 text-xs">
          Space
        </kbd>{" "}
        to flip •{" "}
        <kbd className="rounded bg-slate-100 dark:bg-slate-800 px-2 py-1 text-xs">
          ←
        </kbd>{" "}
        /{" "}
        <kbd className="rounded bg-slate-100 dark:bg-slate-800 px-2 py-1 text-xs">
          →
        </kbd>{" "}
        to navigate
      </div>
    </div>
  );
}
