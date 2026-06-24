"use client";

import { useState } from "react";
import Link from "next/link";
import { FileText, Pencil, Trash2, Check, X, BookOpen } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useMutateRename, useMutateDelete } from "@/hooks/useMutations";
import { useFlashcards } from "@/hooks/useDocuments";
import { DocumentInfo } from "@/lib/types";

interface Props {
  docs: DocumentInfo[];
}

/**
 * Individual document card with its own flashcard count
 *
 * Each card fetches its own flashcard count. This makes N requests
 * for N documents on first load, but TanStack Query caches them, so:
 * - Navigating back to home doesn't re-fetch counts
 * - Deduplication prevents duplicate requests
 *
 * Long-term fix: API should include card_count in document list response.
 */
function DocumentCard({ doc }: { doc: DocumentInfo }) {
  const { data: cards = [], isLoading: countLoading } = useFlashcards(doc.id);
  const cardCount = cards.length;
  const hasCards = cardCount > 0;

  const [editingId, setEditingId] = useState<string | null>(null);
  const [draftName, setDraftName] = useState("");

  const { mutate: rename, isPending: renaming } = useMutateRename();
  const { mutate: remove, isPending: deleting } = useMutateDelete();

  const isBusy = renaming || deleting;
  const isEditing = editingId === doc.id;

  const startRename = () => {
    setEditingId(doc.id);
    setDraftName(doc.filename);
  };

  const cancelRename = () => {
    setEditingId(null);
    setDraftName("");
  };

  const saveRename = () => {
    const trimmed = draftName.trim();
    if (!trimmed) return;
    rename(
      { docId: doc.id, filename: trimmed },
      {
        onSuccess: () => setEditingId(null),
      }
    );
  };

  const handleDelete = () => {
    if (!confirm(`Move "${doc.filename}" to trash? You can restore it later.`)) {
      return;
    }
    remove(doc.id);
  };

  return (
    <Link href={`/document/${doc.id}`} className="block">
      <Card className="transition hover:bg-accent cursor-pointer">
        <CardContent className="flex items-center gap-4 p-4">
          <div className="rounded-full bg-secondary p-2">
            <FileText className="size-5 text-secondary-foreground" />
          </div>
          <div className="min-w-0 flex-1">
            {isEditing ? (
              <div className="flex items-center gap-2">
                <input
                  autoFocus
                  value={draftName}
                  onChange={(e) => setDraftName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") saveRename();
                    if (e.key === "Escape") cancelRename();
                  }}
                  className="h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring"
                  disabled={isBusy}
                />
                <Button
                  size="icon"
                  variant="ghost"
                  onClick={(e) => {
                    e.preventDefault();
                    saveRename();
                  }}
                  disabled={isBusy || !draftName.trim()}
                >
                  <Check className="size-4" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  onClick={(e) => {
                    e.preventDefault();
                    cancelRename();
                  }}
                  disabled={isBusy}
                >
                  <X className="size-4" />
                </Button>
              </div>
            ) : (
              <div>
                <div className="flex items-center gap-2 mb-1">
                  <p className="truncate font-medium">{doc.filename}</p>
                  <Badge variant="secondary" className="uppercase">
                    {doc.file_type}
                  </Badge>
                  {countLoading ? (
                    <Skeleton className="h-5 w-16" />
                  ) : hasCards ? (
                    <Badge variant="outline" className="flex items-center gap-1">
                      <BookOpen className="size-3" />
                      {cardCount}
                    </Badge>
                  ) : null}
                </div>
                <p className="text-sm text-muted-foreground">
                  {doc.page_count} pages ·{" "}
                  {doc.total_chars.toLocaleString()} chars
                </p>
              </div>
            )}
          </div>
          {!isEditing && (
            <div className="flex items-center gap-1">
              {hasCards && (
                <Link href={`/document/${doc.id}`}>
                  <Button
                    variant="default"
                    size="sm"
                    title="Review cards"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <BookOpen className="mr-2 size-4" />
                    Review
                  </Button>
                </Link>
              )}
              <Button
                size="icon"
                variant="ghost"
                onClick={(e) => {
                  e.preventDefault();
                  startRename();
                }}
                disabled={isBusy}
                title="Rename"
              >
                <Pencil className="size-4" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                onClick={(e) => {
                  e.preventDefault();
                  handleDelete();
                }}
                disabled={isBusy}
                title="Move to trash"
              >
                <Trash2 className="size-4" />
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </Link>
  );
}

export default function DocumentList({ docs }: Props) {
  if (docs.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        No documents yet. Upload one above.
      </p>
    );
  }

  return (
    <div className="grid gap-4">
      {docs.map((doc) => (
        <DocumentCard key={doc.id} doc={doc} />
      ))}
    </div>
  );
}
