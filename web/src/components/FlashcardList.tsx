"use client";

import { useState } from "react";
import { Flashcard } from "@/lib/types";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Shuffle } from "lucide-react";

interface Props {
  cards: Flashcard[];
}

export default function FlashcardList({ cards }: Props) {
  const [order, setOrder] = useState<number[]>(() =>
    Array.from({ length: cards.length }, (_, i) => i)
  );
  const [flipped, setFlipped] = useState<Record<string, boolean>>({});

  if (cards.length === 0) {
    return <p className="text-sm text-muted-foreground">No flashcards yet.</p>;
  }

  const toggle = (id: string) => {
    setFlipped((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  const shuffle = () => {
    const next = [...order];
    for (let i = next.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [next[i], next[j]] = [next[j], next[i]];
    }
    setOrder(next);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">
          {cards.length} card{cards.length !== 1 ? "s" : ""}
        </p>
        <Button variant="outline" size="sm" onClick={shuffle}>
          <Shuffle className="mr-2 size-4" />
          Shuffle
        </Button>
      </div>

      {order.map((idx) => {
        const card = cards[idx];
        const isFlipped = flipped[card.id];
        return (
          <Card
            key={card.id}
            onClick={() => toggle(card.id)}
            className="cursor-pointer transition hover:border-primary/50"
          >
            <CardHeader className="pb-2">
              <div className="flex items-center justify-between">
                <Badge variant="outline" className="uppercase">
                  {card.card_type}
                </Badge>
                <span className="text-xs text-muted-foreground">
                  Pages {card.source_ref.page_start}–{card.source_ref.page_end}
                </span>
              </div>
            </CardHeader>
            <CardContent>
              {isFlipped ? (
                <p className="whitespace-pre-wrap text-sm leading-relaxed">
                  {card.answer}
                </p>
              ) : (
                <p className="text-lg font-medium">{card.question}</p>
              )}
              <p className="mt-3 text-xs text-muted-foreground">
                Click to {isFlipped ? "show question" : "reveal answer"}
              </p>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
