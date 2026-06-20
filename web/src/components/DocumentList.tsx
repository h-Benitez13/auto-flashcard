"use client";

import Link from "next/link";

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
    return <p className="text-zinc-500">No documents yet.</p>;
  }

  return (
    <ul className="space-y-3">
      {docs.map((doc) => (
        <li key={doc.id}>
          <Link
            href={`/document/${doc.id}`}
            className="block rounded-xl border p-4 transition hover:bg-zinc-50"
          >
            <div className="flex items-center justify-between">
              <span className="font-medium">{doc.filename}</span>
              <span className="rounded-full bg-zinc-100 px-2 py-1 text-xs uppercase">
                {doc.file_type}
              </span>
            </div>
            <p className="mt-1 text-sm text-zinc-500">
              {doc.page_count} pages · {doc.total_chars.toLocaleString()} chars
            </p>
          </Link>
        </li>
      ))}
    </ul>
  );
}
