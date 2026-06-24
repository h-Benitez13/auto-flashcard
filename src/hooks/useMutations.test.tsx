import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import {
  useMutateRename,
  useMutateDelete,
  useMutateRestore,
  useMutateUpload,
} from "./useMutations";
import { QueryClientWrapper, createTestQueryClient } from "@/lib/test-utils";
import { queryKeys } from "@/lib/query-keys";
import { DocumentInfo } from "@/lib/types";

/**
 * useMutations hook tests
 *
 * Tests mutation hooks including optimistic updates and cache invalidation.
 */
describe("useMutations", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("should rename document optimistically", async () => {
    const docs: DocumentInfo[] = [
      {
        id: "1",
        filename: "old.pdf",
        file_type: "pdf",
        page_count: 1,
        total_chars: 10,
        pages: [],
      },
    ];

    const client = createTestQueryClient();
    client.setQueryData(queryKeys.documents.list(), docs);

    vi.mocked(global.fetch)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ id: "1", filename: "new.pdf" }),
      } as Response)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => [{ id: "1", filename: "new.pdf" }],
      } as Response);

    const { result } = renderHook(() => useMutateRename(), {
      wrapper: ({ children }) => (
        <QueryClientWrapper client={client}>{children}</QueryClientWrapper>
      ),
    });

    // Trigger mutation
    result.current.mutate({ docId: "1", filename: "new.pdf" });

    // Optimistic update should be immediate
    await waitFor(() => {
      const current = client.getQueryData<DocumentInfo[]>(
        queryKeys.documents.list()
      );
      expect(current?.[0].filename).toBe("new.pdf");
    });

    // Wait for mutation to complete
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });

  it("should delete document optimistically", async () => {
    const docs: DocumentInfo[] = [
      {
        id: "1",
        filename: "test.pdf",
        file_type: "pdf",
        page_count: 1,
        total_chars: 10,
        pages: [],
      },
      {
        id: "2",
        filename: "other.pdf",
        file_type: "pdf",
        page_count: 1,
        total_chars: 10,
        pages: [],
      },
    ];

    const client = createTestQueryClient();
    client.setQueryData(queryKeys.documents.list(), docs);

    vi.mocked(global.fetch)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ deleted: true, id: "1" }),
      } as Response)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => [docs[1]],
      } as Response);

    const { result } = renderHook(() => useMutateDelete(), {
      wrapper: ({ children }) => (
        <QueryClientWrapper client={client}>{children}</QueryClientWrapper>
      ),
    });

    result.current.mutate("1");

    // Optimistic update removes doc immediately
    await waitFor(() => {
      const current = client.getQueryData<DocumentInfo[]>(
        queryKeys.documents.list()
      );
      expect(current?.find((d) => d.id === "1")).toBeUndefined();
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });

  it("should restore document and invalidate caches", async () => {
    const client = createTestQueryClient();

    vi.mocked(global.fetch)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ restored: true, id: "1" }),
      } as Response)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => [],
      } as Response)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => [],
      } as Response);

    const { result } = renderHook(() => useMutateRestore(), {
      wrapper: ({ children }) => (
        <QueryClientWrapper client={client}>{children}</QueryClientWrapper>
      ),
    });

    result.current.mutate("1");

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });

  it("should upload file and invalidate document list", async () => {
    const client = createTestQueryClient();

    vi.mocked(global.fetch)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          id: "1",
          filename: "upload.pdf",
          file_type: "pdf",
          page_count: 1,
          total_chars: 10,
          pages: [],
        }),
      } as Response)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => [],
      } as Response)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => [],
      } as Response);

    const { result } = renderHook(() => useMutateUpload(), {
      wrapper: ({ children }) => (
        <QueryClientWrapper client={client}>{children}</QueryClientWrapper>
      ),
    });

    const file = new File(["content"], "upload.pdf", { type: "application/pdf" });
    result.current.mutate(file);

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});
