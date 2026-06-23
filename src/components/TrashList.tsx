"use client";

import { useState } from "react";
import { RotateCcw, FileText } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { restoreDocument } from "@/lib/api";

interface Doc {
  id: string;
  filename: string;
  file_type: string;
  page_count: number;
  total_chars: number;
  deleted_at?: string | null;
}

interface Props {
  docs: Doc[];
  onChanged?: () => void;
}

function formatDeletedAt(value?: string | null): string {
  if (!value) return "";
  try {
    const d = new Date(value);
    return d.toLocaleString();
  } catch {
    return value;
  }
}

export default function TrashList({ docs, onChanged }: Props) {
  const [busyId, setBusyId] = useState<string | null>(null);

  const handleRestore = async (doc: Doc) => {
    setBusyId(doc.id);
    try {
      await restoreDocument(doc.id);
      onChanged?.();
    } catch {
      // ignore for now
    } finally {
      setBusyId(null);
    }
  };

  if (docs.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">Trash is empty.</p>
    );
  }

  return (
    <div className="grid gap-4">
      {docs.map((doc) => {
        const isBusy = busyId === doc.id;
        return (
          <Card key={doc.id} className="opacity-75">
            <CardContent className="flex items-center gap-4 p-4">
              <div className="rounded-full bg-secondary p-2">
                <FileText className="size-5 text-secondary-foreground" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <p className="truncate font-medium">{doc.filename}</p>
                  <Badge variant="secondary" className="uppercase">
                    {doc.file_type}
                  </Badge>
                </div>
                <p className="text-sm text-muted-foreground">
                  {doc.page_count} pages ·{" "}
                  {doc.total_chars.toLocaleString()} chars
                  {doc.deleted_at && (
                    <> · deleted {formatDeletedAt(doc.deleted_at)}</>
                  )}
                </p>
              </div>
              <Button
                size="sm"
                variant="outline"
                onClick={() => handleRestore(doc)}
                disabled={isBusy}
              >
                <RotateCcw className="mr-2 size-4" />
                Restore
              </Button>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
