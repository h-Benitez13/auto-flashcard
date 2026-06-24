/**
 * Query key factory
 *
 * Centralized place to define all query keys.
 * Prevents typos, makes cache invalidation explicit.
 *
 * Usage:
 *   useQuery({ queryKey: queryKeys.documents.all, ... })
 *   queryClient.invalidateQueries({ queryKey: queryKeys.documents.all })
 */

export const queryKeys = {
  documents: {
    all: ["documents"] as const,
    lists: () => [{ ...queryKeys.documents.all, scope: "list" }] as const,
    list: () => queryKeys.documents.lists(),
    details: () => [{ ...queryKeys.documents.all, scope: "detail" }] as const,
    detail: (id: string) =>
      [{ ...queryKeys.documents.details(), id }] as const,
  },

  flashcards: {
    all: ["flashcards"] as const,
    lists: () => [{ ...queryKeys.flashcards.all, scope: "list" }] as const,
    list: (docId: string) =>
      [{ ...queryKeys.flashcards.lists(), docId }] as const,
  },

  jobs: {
    all: ["jobs"] as const,
    details: () => [{ ...queryKeys.jobs.all, scope: "detail" }] as const,
    detail: (jobId: string) =>
      [{ ...queryKeys.jobs.details(), jobId }] as const,
  },

  trash: {
    all: ["trash"] as const,
  },
} as const;
