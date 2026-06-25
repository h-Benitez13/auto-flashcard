/**
 * Query hooks for document data
 *
 * These hooks manage server state (documents, flashcards, jobs)
 * via TanStack Query. No manual state management needed.
 */

import { useQuery } from "@tanstack/react-query";
import { apiClient } from "@/lib/api-client";
import { queryKeys } from "@/lib/query-keys";
import { DocumentInfo, Flashcard, GenerationJob } from "@/lib/types";

/**
 * Fetch all active documents
 *
 * Usage:
 *   const { data: docs, isLoading, error } = useDocuments();
 *
 * Returns:
 *   - data: Document[] (auto-refetches if stale)
 *   - isLoading: true while fetching
 *   - error: null or Error object
 *   - refetch: manual refetch function
 */
export function useDocuments() {
  return useQuery({
    queryKey: queryKeys.documents.list(),
    queryFn: async () => {
      return apiClient.get<DocumentInfo[]>("/documents");
    },
  });
}

/**
 * Fetch a single document with full details
 *
 * Usage:
 *   const { data: doc, isLoading } = useDocument(docId);
 *
 * Returns:
 *   - data: DocumentInfo (includes pages)
 *   - isLoading: true while fetching
 *   - error: null or Error (including 404)
 */
export function useDocument(id: string) {
  return useQuery({
    queryKey: queryKeys.documents.detail(id),
    queryFn: async () => {
      return apiClient.get<DocumentInfo>(`/documents/${id}`);
    },
    // Don't refetch in background for a specific doc
    staleTime: Infinity,
  });
}

/**
 * Fetch flashcards for a document
 *
 * Usage:
 *   const { data: cards } = useFlashcards(docId);
 *
 * Automatically invalidated when generation completes.
 */
export function useFlashcards(docId: string) {
  return useQuery({
    queryKey: queryKeys.flashcards.list(docId),
    queryFn: async () => {
      return apiClient.get<Flashcard[]>(`/documents/${docId}/flashcards`);
    },
    enabled: !!docId, // Don't fetch if docId is empty
  });
}

/**
 * Fetch a generation job (for polling)
 *
 * Usage in useGenerationJob hook (see next file):
 *   const job = useQuery({
 *     queryKey: queryKeys.jobs.detail(jobId),
 *     queryFn: () => apiClient.get(`/jobs/${jobId}`),
 *   });
 */
export function useGenerationJob(jobId: string) {
  return useQuery({
    queryKey: queryKeys.jobs.detail(jobId),
    queryFn: async () => {
      return apiClient.get<GenerationJob>(`/jobs/${jobId}`);
    },
    enabled: !!jobId,
    staleTime: 0, // Always refetch job status (for polling)
  });
}

/**
 * Fetch trash items
 */
export function useTrash() {
  return useQuery({
    queryKey: queryKeys.trash.all,
    queryFn: async () => {
      return apiClient.get<DocumentInfo[]>("/trash");
    },
  });
}
