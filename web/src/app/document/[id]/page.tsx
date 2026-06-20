"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { getDocument } from "@/lib/api";
import { DocumentInfo } from "@/lib/types";

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
  if (error) return <p className="p-8 text-red-600">{error}</p>;
  if (!doc) return null;

  return (
    <main className="mx-auto max-w-4xl p-8">
      <Link href="/" className="text-sm text-sky-600 hover:underline">
        ← Back
      </Link>
      <h1 className="mb-2 mt-4 text-3xl font-bold">{doc.filename}</h1>
      <p className="mb-8 text-zinc-600">
        {doc.page_count} pages · {doc.total_chars.toLocaleString()} chars
      </p>

      <div className="space-y-6">
        {doc.pages.map((page) => (
          <section key={page.page_num} className="rounded-xl border p-4">
            <h3 className="mb-2 text-sm font-semibold uppercase text-zinc-500">
              Page {page.page_num}
            </h3>
            <pre className="whitespace-pre-wrap font-sans text-sm leading-relaxed text-zinc-800">
              {page.text}
            </pre>
          </section>
        ))}
      </div>
    </main>
  );
}
