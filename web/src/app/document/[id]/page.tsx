"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { ArrowLeft, FileText } from "lucide-react";

import { getDocument } from "@/lib/api";
import { DocumentInfo } from "@/lib/types";
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

export default function DocumentPage() {
  const { id } = useParams<{ id: string }>();
  const [doc, setDoc] = useState<DocumentInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    getDocument(id)
      .then(setDoc)
      .catch(() => setError("Could not load document"))
      .finally(() => setLoading(false));
  }, [id]);

  if (loading) return <p className="p-8">Loading…</p>;
  if (error) return <p className="p-8 text-destructive">{error}</p>;
  if (!doc) return null;

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
            <CardDescription>
              <Badge variant="secondary" className="mr-2 uppercase">
                {doc.file_type}
              </Badge>
              {doc.page_count} pages · {doc.total_chars.toLocaleString()} chars
            </CardDescription>
          </div>
        </CardHeader>
      </Card>

      <div className="space-y-4">
        {doc.pages.map((page, idx) => (
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
    </main>
  );
}
