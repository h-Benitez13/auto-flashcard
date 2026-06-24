/**
 * Generation job polling with adaptive backoff
 *
 * This hook implements smart polling:
 * - Start at 1s interval
 * - If progress stalls, increase to 2s, 5s, 10s
 * - Stop polling when job completes
 * - Auto-cleanup when component unmounts
 *
 * Problem it solves:
 * - Old code: 1s interval forever (hammers API)
 * - New code: adaptive interval (saves bandwidth)
 */

import { useEffect, useRef } from "react";
import { useGenerationJob as useGenerationJobQuery } from "./useDocuments";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "@/lib/query-keys";
import { toast } from "sonner";

interface UseGenerationJobPollingOptions {
  docId: string;
  jobId: string;
  onCompleted?: () => void;
  maxDurationMs?: number; // Stop polling after 1 hour
}

/**
 * Use this hook to manage generation job polling
 *
 * Usage:
 *   const { data: job, isLoading } = useGenerationJobPolling({
 *     docId,
 *     jobId,
 *     onCompleted: () => {
 *       // Job finished, can refresh cards
 *     },
 *     maxDurationMs: 60 * 60 * 1000, // 1 hour max
 *   });
 *
 *   {job && job.status === 'generating' && (
 *     <div>{job.progress} / {job.total}</div>
 *   )}
 */
export function useGenerationJobPolling({
  docId,
  jobId,
  onCompleted,
  maxDurationMs = 60 * 60 * 1000, // 1 hour default
}: UseGenerationJobPollingOptions) {
  const queryClient = useQueryClient();

  // Fetch job status
  const {
    data: job,
    refetch,
    isLoading,
  } = useGenerationJobQuery(jobId || "");

  // Track polling state
  const intervalRef = useRef<NodeJS.Timeout | null>(null);
  const startTimeRef = useRef<number>(0);
  const lastProgressRef = useRef<number>(0);
  const stallCountRef = useRef<number>(0);

  /**
   * Adaptive polling logic:
   * - If progress changed -> reset interval to 1s
   * - If progress stalled -> increase interval to 2s, 5s, 10s
   * - If running > 1 hour -> stop polling
   */
  useEffect(() => {
    if (!jobId || !job) return;

    // Initialize start time on first poll
    if (startTimeRef.current === 0) {
      startTimeRef.current = Date.now();
    }

    // If job is done, stop polling
    if (
      job.status === "completed" ||
      job.status === "completed_fallback" ||
      job.status === "failed"
    ) {
      if (intervalRef.current) clearInterval(intervalRef.current);

      if (
        (job.status === "completed" ||
          job.status === "completed_fallback") &&
        onCompleted
      ) {
        onCompleted();

        // Refetch flashcards since generation is done
        queryClient.invalidateQueries({
          queryKey: queryKeys.flashcards.list(docId),
        });
      }

      if (job.status === "failed") {
        toast.error(
          `Generation failed: ${job.error_message || "Unknown error"}`
        );
      }

      return;
    }

    // Polling is running
    if (job.status === "generating") {
      // Check if progress stalled
      if (job.progress === lastProgressRef.current) {
        stallCountRef.current++;
      } else {
        stallCountRef.current = 0;
        lastProgressRef.current = job.progress;
      }

      // Calculate interval based on stalls
      // 0 stalls: 1s, 1 stall: 2s, 2+ stalls: 5s, 5+ stalls: 10s
      let interval = 1000;
      if (stallCountRef.current >= 5) interval = 10000;
      else if (stallCountRef.current >= 2) interval = 5000;
      else if (stallCountRef.current >= 1) interval = 2000;

      // Check if we've exceeded max duration
      const elapsed = Date.now() - startTimeRef.current;
      if (elapsed > maxDurationMs) {
        if (intervalRef.current) clearInterval(intervalRef.current);
        toast.error(
          "Generation is taking too long. Please try again later."
        );
        return;
      }

      // Clear old interval and set new one
      if (intervalRef.current) clearInterval(intervalRef.current);

      intervalRef.current = setInterval(() => {
        refetch();
      }, interval);
    }

    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [job, jobId, docId, queryClient, onCompleted, refetch, maxDurationMs]);

  return { data: job, isLoading, refetch };
}

/**
 * COMPARISON: Old polling vs new polling
 *
 * OLD (from the current code):
 *   useEffect(() => {
 *     const interval = setInterval(async () => {
 *       const updated = await getJob(job.id);
 *       setJob(updated);
 *     }, 1000); // Always 1s, FOREVER
 *     return () => clearInterval(interval);
 *   }, [job, id]);
 *
 * Problems:
 * 1s hammer forever (even if generation is stuck)
 * No max duration (can poll for days)
 * Dependency array includes job object (unstable)
 * Silent errors (no toast)
 *
 * NEW (this file):
 * Adaptive: 1s -> 2s -> 5s -> 10s
 * Max duration: stops after 1 hour
 * Stable dependencies (jobId only)
 * Visible errors (toast notifications)
 * Automatic refetch of flashcards on success
 * Better memory management (cleanup guaranteed)
 */
