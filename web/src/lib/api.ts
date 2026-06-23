import { DocumentInfo, Flashcard, GenerationJob } from "./types";

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

export interface GenerateOptions {
  density?: "concise" | "balanced" | "comprehensive";
  page_numbers?: number[];
}

export async function generateFlashcards(
  id: string,
  opts: GenerateOptions = {}
): Promise<{ job_id: string; total_chunks: number; use_llm: boolean }> {
  const res = await fetch(`${API_URL}/documents/${id}/generate`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(opts),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Generation failed: ${res.status}`);
  }
  return res.json();
}

export async function getJob(id: string): Promise<GenerationJob> {
  const res = await fetch(`${API_URL}/jobs/${id}`);
  if (!res.ok) throw new Error("Failed to load job");
  return res.json();
}

export async function getFlashcards(id: string): Promise<Flashcard[]> {
  const res = await fetch(`${API_URL}/documents/${id}/flashcards`);
  if (!res.ok) throw new Error("Failed to load flashcards");
  return res.json();
}

export async function renameDocument(
  id: string,
  filename: string
): Promise<{ id: string; filename: string }> {
  const res = await fetch(`${API_URL}/documents/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ filename }),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Rename failed: ${res.status}`);
  }
  return res.json();
}

export async function deleteDocument(
  id: string
): Promise<{ deleted: boolean; id: string }> {
  const res = await fetch(`${API_URL}/documents/${id}`, { method: "DELETE" });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Delete failed: ${res.status}`);
  }
  return res.json();
}

export async function restoreDocument(
  id: string
): Promise<{ restored: boolean; id: string }> {
  const res = await fetch(`${API_URL}/documents/${id}/restore`, {
    method: "POST",
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Restore failed: ${res.status}`);
  }
  return res.json();
}

export async function listTrash(): Promise<DocumentSummary[]> {
  const res = await fetch(`${API_URL}/trash`);
  if (!res.ok) throw new Error("Failed to load trash");
  return res.json();
}
