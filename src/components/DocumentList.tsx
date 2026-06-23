"use client";

import { useState } from "react";
import Link from "next/link";
import { FileText, Pencil, Trash2, Check, X, BookOpen } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { renameDocument, deleteDocument } from "@/lib/api";

interface Doc {
  id: string;
  filename: string;
  file_type: string;
  page_count: number;
  total_chars: number;
}

interface Props {
  docs: Doc[];
  cardCounts: Record<string, number>;
  onChanged?: () => void;
}

export default function DocumentList({ docs, cardCounts, onChanged }: Props) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draftName, setDraftName] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);

  const startRename = (doc: Doc) => {
    setEditingId(doc.id);
    setDraftName(doc.filename);
  };

  const cancelRename = () => {
    setEditingId(null);
    setDraftName("");
  };

  const saveRename = async (id: string) => {
    const trimmed = draftName.trim();
    if (!trimmed) return;
    setBusyId(id);
    try {
      await renameDocument(id, trimmed);
      setEditingId(null);
      onChanged?.();
    } catch {
      // keep editing on failure
    } finally {
      setBusyId(null);
    }
  };

  const handleDelete = async (doc: Doc) => {
    if (!confirm(`Move "${doc.filename}" to trash? You can restore it later.`)) {
      return;
    }
    setBusyId(doc.id);
    try {
      await deleteDocument(doc.id);
      onChanged?.();
    } catch {
      // ignore for now
    } finally {
      setBusyId(null);
    }
  };

  if (docs.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        No documents yet. Upload one above.
      </p>
    );
  }

  return (
    <div className="grid gap-4">
      {docs.map((doc) => {
        const isEditing = editingId === doc.id;
        const isBusy = busyId === doc.id;
        const cardCount = cardCounts[doc.id] ?? 0;
        const hasCards = cardCount > 0;

        return (
          <Card key={doc.id} className="transition hover:bg-accent">
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
                        if (e.key === "Enter") saveRename(doc.id);
                        if (e.key === "Escape") cancelRename();
                      }}
                      className="h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring"
                      disabled={isBusy}
                    />
                    <Button
                      size="icon"
                      variant="ghost"
                      onClick={() => saveRename(doc.id)}
                      disabled={isBusy || !draftName.trim()}
                    >
                      <Check className="size-4" />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      onClick={cancelRename}
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
                      {hasCards && (
                        <Badge variant="outline" className="flex items-center gap-1">
                          <BookOpen className="size-3" />
                          {cardCount}
                        </Badge>
                      )}
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
                      >
                        <BookOpen className="mr-2 size-4" />
                        Review
                      </Button>
                    </Link>
                  )}
                  <Button
                    size="icon"
                    variant="ghost"
                    onClick={() => startRename(doc)}
                    disabled={isBusy}
                    title="Rename"
                  >
                    <Pencil className="size-4" />
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    onClick={() => handleDelete(doc)}
                    disabled={isBusy}
                    title="Move to trash"
                  >
                    <Trash2 className="size-4" />
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
