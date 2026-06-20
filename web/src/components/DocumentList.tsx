"use client";

import Link from "next/link";
import { FileText } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

interface Doc {
  id: string;
  filename: string;
  file_type: string;
  page_count: number;
  total_chars: number;
}

interface Props {
  docs: Doc[];
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
        <Link key={doc.id} href={`/document/${doc.id}`}>
          <Card className="transition hover:bg-accent">
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
                  {doc.page_count} pages · {doc.total_chars.toLocaleString()}{" "}
                  chars
                </p>
              </div>
            </CardContent>
          </Card>
        </Link>
      ))}
    </div>
  );
}
