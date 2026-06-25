/**
 * Mutation hooks for mutations (POST, PATCH, DELETE)
 *
 * Mutations handle:
 * 1. Sending request
 * 2. Optimistic updates (show result immediately)
 * 3. Automatic cache invalidation (refetch related queries)
 * 4. Error handling + toast notifications
 *
 * Key difference vs TanStack Query queries:
 * - Queries: GET data, auto-refetch, caching
 * - Mutations: POST/PATCH/DELETE data, one-time, handle side effects
 */

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { apiClient } from "@/lib/api-client";
import { queryKeys } from "@/lib/query-keys";
import { DocumentInfo } from "@/lib/types";
import { toast } from "sonner";

/**
 * Upload a file
 *
 * Usage:
 *   const { mutate, isPending } = useMutateUpload();
 *   <button onClick={() => mutate(file)} disabled={isPending}>
 *     {isPending ? 'Uploading...' : 'Upload'}
 *   </button>
 *
 * Side effects:
 * - On success: invalidate documents list + trash (user can see new doc)
 * - On error: show toast notification
 */
export function useMutateUpload() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (file: File) => {
      const form = new FormData();
      form.append("file", file);

      const res = await fetch(
        `${process.env.NEXT_PUBLIC_API_URL || "http://localhost:3001"}/upload`,
        {
          method: "POST",
          body: form,
        }
      );

      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(
          body.error || `Upload failed: ${res.status}`
        );
      }

      return res.json() as Promise<DocumentInfo>;
    },

    // On success
    onSuccess: () => {
      // Refetch documents list so user sees new upload
      queryClient.invalidateQueries({
        queryKey: queryKeys.documents.all,
      });
      // Also update trash in case it was restored
      queryClient.invalidateQueries({
        queryKey: queryKeys.trash.all,
      });
      toast.success("File uploaded successfully");
    },

    // On error
    onError: (error) => {
      const message =
        error instanceof Error ? error.message : "Upload failed";
      toast.error(message);
    },
  });
}

/**
 * Rename a document
 *
 * Usage:
 *   const { mutate, isPending } = useMutateRename();
 *   mutate({ docId: '123', filename: 'New Name' });
 *
 * Side effects:
 * - Optimistic update: show new name immediately
 * - On success: confirm via refetch
 * - On error: revert to old name, show toast
 */
export function useMutateRename() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      docId,
      filename,
    }: {
      docId: string;
      filename: string;
    }) => {
      return apiClient.patch<DocumentInfo>(
        `/documents/${docId}`,
        { filename }
      );
    },

    // Optimistic update: show new name immediately
    // (before server confirms)
    onMutate: async ({ docId, filename }) => {
      // Cancel any pending refetches
      await queryClient.cancelQueries({
        queryKey: queryKeys.documents.all,
      });

      // Get old data
      const previousDocs = queryClient.getQueryData(
        queryKeys.documents.list()
      );

      // Update cache optimistically
      queryClient.setQueryData(
        queryKeys.documents.list(),
        (old: DocumentInfo[] | undefined) =>
          old
            ? old.map((d) =>
                d.id === docId ? { ...d, filename } : d
              )
            : undefined
      );

      return { previousDocs };
    },

    // If error, rollback to old data
    onError: (error, _variables, context) => {
      if (context?.previousDocs) {
        queryClient.setQueryData(
          queryKeys.documents.list(),
          context.previousDocs
        );
      }
      const message =
        error instanceof Error
          ? error.message
          : "Rename failed";
      toast.error(message);
    },

    // On success, refetch to confirm
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.documents.all,
      });
      toast.success("Document renamed");
    },
  });
}

/**
 * Delete a document (move to trash)
 *
 * Usage:
 *   const { mutate, isPending } = useMutateDelete();
 *   mutate('doc-id-123');
 *
 * Side effects:
 * - Optimistic: remove from documents list immediately
 * - On success: refetch documents + trash
 * - On error: restore to documents list, show toast
 */
export function useMutateDelete() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (docId: string) => {
      return apiClient.delete<{ deleted: boolean }>(
        `/documents/${docId}`
      );
    },

    // Optimistic: remove from list
    onMutate: async (docId) => {
      await queryClient.cancelQueries({
        queryKey: queryKeys.documents.all,
      });

      const previousDocs = queryClient.getQueryData(
        queryKeys.documents.list()
      );

      queryClient.setQueryData(
        queryKeys.documents.list(),
        (old: DocumentInfo[] | undefined) =>
          old ? old.filter((d) => d.id !== docId) : undefined
      );

      return { previousDocs };
    },

    onError: (error, _variables, context) => {
      if (context?.previousDocs) {
        queryClient.setQueryData(
          queryKeys.documents.list(),
          context.previousDocs
        );
      }
      toast.error("Delete failed");
    },

    onSuccess: () => {
      // Refetch docs and trash
      queryClient.invalidateQueries({
        queryKey: queryKeys.documents.all,
      });
      queryClient.invalidateQueries({
        queryKey: queryKeys.trash.all,
      });
      toast.success("Moved to trash");
    },
  });
}

/**
 * Restore a document from trash
 *
 * Usage:
 *   const { mutate } = useMutateRestore();
 *   mutate('doc-id-123');
 */
export function useMutateRestore() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (docId: string) => {
      return apiClient.post(`/documents/${docId}/restore`);
    },

    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.documents.all,
      });
      queryClient.invalidateQueries({
        queryKey: queryKeys.trash.all,
      });
      toast.success("Document restored");
    },

    onError: () => {
      toast.error("Restore failed");
    },
  });
}

/**
 * Generate flashcards
 *
 * Usage:
 *   const { mutate } = useMutateGenerate();
 *   const job = mutate({ docId, density: 'balanced' });
 */
export function useMutateGenerate() {
  return useMutation({
    mutationFn: async ({
      docId,
      density,
    }: {
      docId: string;
      density: "concise" | "balanced" | "comprehensive";
    }) => {
      const result = await apiClient.post<{
        job_id: string;
        total_chunks: number;
        use_llm: boolean;
      }>(`/documents/${docId}/generate`, { density });

      // Fetch the job immediately to set up polling
      const job = await apiClient.get(
        `/jobs/${result.job_id}`
      );

      return { jobId: result.job_id, job };
    },

    onError: (error) => {
      const message =
        error instanceof Error
          ? error.message
          : "Generation failed";
      toast.error(message);
    },
  });
}
