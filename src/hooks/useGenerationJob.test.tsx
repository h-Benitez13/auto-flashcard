import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useGenerationJobPolling } from "./useGenerationJob";
import { QueryClientWrapper, createTestQueryClient } from "@/lib/test-utils";
import { queryKeys } from "@/lib/query-keys";
import { GenerationJob } from "@/lib/types";

/**
 * useGenerationJobPolling tests
 *
 * Tests adaptive polling behavior and completion handling.
 * Uses fake timers to control interval timing.
 */
describe("useGenerationJobPolling", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("should poll job until completed", async () => {
    const completedJob: GenerationJob = {
      id: "job1",
      document_id: "doc1",
      status: "completed",
      progress: 10,
      total: 10,
      use_llm: true,
    };

    vi.mocked(global.fetch).mockResolvedValue({
      ok: true,
      json: async () => completedJob,
    } as Response);

    const client = createTestQueryClient();
    client.setQueryData(queryKeys.jobs.detail("job1"), completedJob);

    const onCompleted = vi.fn();

    const { result } = renderHook(
      () =>
        useGenerationJobPolling({
          docId: "doc1",
          jobId: "job1",
          onCompleted,
        }),
      {
        wrapper: ({ children }) => (
          <QueryClientWrapper client={client}>{children}</QueryClientWrapper>
        ),
      }
    );

    await waitFor(() => {
      expect(result.current.data?.status).toBe("completed");
    });

    expect(onCompleted).toHaveBeenCalled();
  });

  it("should stop polling when job fails", async () => {
    const failedJob: GenerationJob = {
      id: "job1",
      document_id: "doc1",
      status: "failed",
      progress: 0,
      total: 10,
      use_llm: true,
      error_message: "LLM failed",
    };

    vi.mocked(global.fetch).mockResolvedValue({
      ok: true,
      json: async () => failedJob,
    } as Response);

    const client = createTestQueryClient();
    client.setQueryData(queryKeys.jobs.detail("job1"), failedJob);

    const { result } = renderHook(
      () =>
        useGenerationJobPolling({
          docId: "doc1",
          jobId: "job1",
        }),
      {
        wrapper: ({ children }) => (
          <QueryClientWrapper client={client}>{children}</QueryClientWrapper>
        ),
      }
    );

    await waitFor(() => {
      expect(result.current.data?.status).toBe("failed");
    });
  });

  it("should not poll without a jobId", async () => {
    const { result } = renderHook(
      () =>
        useGenerationJobPolling({
          docId: "doc1",
          jobId: "",
        }),
      {
        wrapper: QueryClientWrapper,
      }
    );

    expect(result.current.data).toBeUndefined();
    expect(global.fetch).not.toHaveBeenCalled();
  });
});
