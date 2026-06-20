import { DocumentInfo } from "./types";

export const API_URL =
  process.env.NEXT_PUBLIC_API_URL || "http://localhost:3001";

export async function uploadFile(file: File): Promise<DocumentInfo> {
  const form = new FormData();
  form.append("file", file);
  const res = await fetch(`${API_URL}/upload`, {
    method: "POST",
    body: form,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Upload failed: ${res.status}`);
  }
  return res.json();
}

export type DocumentSummary = Pick<
  DocumentInfo,
  "id" | "filename" | "file_type" | "page_count" | "total_chars"
>;

export async function listDocuments(): Promise<DocumentSummary[]> {
  const res = await fetch(`${API_URL}/documents`);
  if (!res.ok) throw new Error("Failed to load documents");
  return res.json();
}

export async function getDocument(id: string): Promise<DocumentInfo> {
  const res = await fetch(`${API_URL}/documents/${id}`);
  if (!res.ok) throw new Error("Failed to load document");
  return res.json();
}
