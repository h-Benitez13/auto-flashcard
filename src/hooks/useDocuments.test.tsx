import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useDocuments, useDocument, useFlashcards } from "./useDocuments";
import { QueryClientWrapper, createTestQueryClient } from "@/lib/test-utils";
import { DocumentInfo, Flashcard } from "@/lib/types";

/**
 * useDocuments hook tests
 *
 * Tests TanStack Query data fetching hooks.
 * Each test uses a fresh QueryClient to avoid cache pollution.
 */
describe("useDocuments", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("should fetch list of documents", async () => {
    const mockDocs: DocumentInfo[] = [
      {
        id: "1",
        filename: "test.pdf",
        file_type: "pdf",
        page_count: 2,
        total_chars: 100,
        pages: [],
      },
    ];

    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: true,
      json: async () => mockDocs,
    } as Response);

    const { result } = renderHook(() => useDocuments(), {
      wrapper: QueryClientWrapper,
    });

    // Initially loading
    expect(result.current.isLoading).toBe(true);

    // Wait for data
    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(result.current.data).toEqual(mockDocs);
  });

  it("should return error state on fetch failure", async () => {
    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: false,
      status: 500,
      json: async () => ({ error: "Server error" }),
    } as Response);

    const { result } = renderHook(() => useDocuments(), {
      wrapper: QueryClientWrapper,
    });

    await waitFor(() => {
      expect(result.current.isError).toBe(true);
    });

    expect(result.current.error).toBeDefined();
  });

  it("should fetch a single document", async () => {
    const mockDoc: DocumentInfo = {
      id: "1",
      filename: "test.pdf",
      file_type: "pdf",
      page_count: 2,
      total_chars: 100,
      pages: [
        { page_num: 1, text: "Hello", char_offset: 0 },
        { page_num: 2, text: "World", char_offset: 6 },
      ],
    };

    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: true,
      json: async () => mockDoc,
    } as Response);

    const { result } = renderHook(() => useDocument("1"), {
      wrapper: QueryClientWrapper,
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(result.current.data).toEqual(mockDoc);
  });

  it("should fetch flashcards for a document", async () => {
    const mockCards: Flashcard[] = [
      {
        id: "c1",
        document_id: "1",
        chunk_id: "ch1",
        question: "Q1?",
        answer: "A1",
        card_type: "concept",
        source_ref: {
          page_start: 1,
          page_end: 1,
          char_start: 0,
          char_end: 10,
          preview: "preview text",
        },
        tags: [],
        provider: "llm",
      },
    ];

    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: true,
      json: async () => mockCards,
    } as Response);

    const { result } = renderHook(() => useFlashcards("1"), {
      wrapper: QueryClientWrapper,
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(result.current.data).toEqual(mockCards);
  });

  it("should cache document data and not refetch", async () => {
    const mockDoc: DocumentInfo = {
      id: "1",
      filename: "test.pdf",
      file_type: "pdf",
      page_count: 1,
      total_chars: 10,
      pages: [],
    };

    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: true,
      json: async () => mockDoc,
    } as Response);

    const client = createTestQueryClient();

    // First render fetches
    const { result: result1 } = renderHook(() => useDocument("1"), {
      wrapper: ({ children }) => (
        <QueryClientWrapper client={client}>{children}</QueryClientWrapper>
      ),
    });

    await waitFor(() => expect(result1.current.isSuccess).toBe(true));

    // Second render with same client should use cache
    const { result: result2 } = renderHook(() => useDocument("1"), {
      wrapper: ({ children }) => (
        <QueryClientWrapper client={client}>{children}</QueryClientWrapper>
      ),
    });

    expect(result2.current.data).toEqual(mockDoc);
    expect(global.fetch).toHaveBeenCalledTimes(1);
  });
});
