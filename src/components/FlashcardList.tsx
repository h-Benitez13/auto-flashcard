"use client";

import { useState } from "react";
import { Flashcard } from "@/lib/types";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Shuffle, LayoutGrid } from "lucide-react";

interface Props {
  cards: Flashcard[];
}

type GridColumns = 1 | 2 | 3;

function providerLabel(provider?: Flashcard["provider"]) {
  if (provider === "rule-based") return "Rule-based";
  if (provider === "llm") return "LLM";
  return undefined;
}

export default function FlashcardList({ cards }: Props) {
  const [order, setOrder] = useState<number[]>(() =>
    Array.from({ length: cards.length }, (_, i) => i)
  );
  const [flipped, setFlipped] = useState<Record<string, boolean>>({});
  const [columns, setColumns] = useState<GridColumns>(1);

  if (cards.length === 0) {
    return <p className="text-sm text-muted-foreground">No flashcards yet.</p>;
  }

  const shuffle = () => {
    const next = [...order];
    for (let i = next.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [next[i], next[j]] = [next[j], next[i]];
    }
    setOrder(next);
    setFlipped({});
  };

  const toggleFlip = (cardId: string) => {
    setFlipped((prev) => ({ ...prev, [cardId]: !prev[cardId] }));
  };

  const gridClass =
    columns === 3
      ? "grid-cols-1 md:grid-cols-3"
      : columns === 2
        ? "grid-cols-1 md:grid-cols-2"
        : "grid-cols-1";

  return (
    <div className="space-y-6">
      {/* Header with controls */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <Button variant="outline" size="sm" onClick={shuffle}>
          <Shuffle className="mr-2 size-4" />
          Shuffle
        </Button>

        <div className="flex items-center gap-2">
          <LayoutGrid className="size-4 text-muted-foreground" />
          <div className="flex items-center rounded-md border bg-background p-1">
            {[1, 2, 3].map((col) => (
              <button
                key={col}
                onClick={() => setColumns(col as GridColumns)}
                className={`rounded px-3 py-1 text-xs font-medium transition-colors ${
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
      </div>

      {/* Cards grid */}
      <div className={`grid ${gridClass} gap-4`}>
        {order.map((cardIdx) => {
          const card = cards[cardIdx];
          if (!card) return null;

          const isFlipped = flipped[card.id] ?? false;
          const rawPreview = card.source_ref?.preview ?? "";
          const preview =
            rawPreview.length > 200
              ? rawPreview.substring(0, 200) + "..."
              : rawPreview;
          const provider = providerLabel(card.provider);

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
                    {provider && (
                      <Badge
                        variant={
                          card.provider === "rule-based" ? "secondary" : "default"
                        }
                        className="text-xs"
                      >
                        {provider}
                      </Badge>
                    )}
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
        Click any card to flip • Use the grid toggle above to change layout
      </div>
    </div>
  );
}
