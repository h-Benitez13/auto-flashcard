"use client";

import { RotateCcw, FileText } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useMutateRestore } from "@/hooks/useMutations";
import { DocumentInfo } from "@/lib/types";

interface Props {
  docs: DocumentInfo[];
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

function TrashItem({ doc }: { doc: DocumentInfo & { deleted_at?: string | null } }) {
  const { mutate: restore, isPending: restoring } = useMutateRestore();

  const handleRestore = () => {
    restore(doc.id);
  };

  return (
    <Card className="opacity-75">
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
          onClick={handleRestore}
          disabled={restoring}
        >
          <RotateCcw className="mr-2 size-4" />
          Restore
        </Button>
      </CardContent>
    </Card>
  );
}

export default function TrashList({ docs }: Props) {
  if (docs.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">Trash is empty.</p>
    );
  }

  return (
    <div className="grid gap-4">
      {docs.map((doc) => (
        <TrashItem key={doc.id} doc={doc} />
      ))}
    </div>
  );
}
