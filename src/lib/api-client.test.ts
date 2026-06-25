import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { apiClient, ApiError } from "./api-client";

/**
 * ApiClient tests
 *
 * Tests the HTTP client layer:
 * - GET/POST/PATCH/DELETE methods
 * - Error handling
 * - JSON parsing
 */
describe("apiClient", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("should make a GET request and parse JSON", async () => {
    const mockData = { id: "1", filename: "test.pdf" };
    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: true,
      json: async () => mockData,
    } as Response);

    const result = await apiClient.get("/documents/1");
    expect(result).toEqual(mockData);
    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining("/documents/1"),
      expect.objectContaining({ method: "GET" })
    );
  });

  it("should throw ApiError on HTTP error", async () => {
    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: false,
      status: 404,
      json: async () => ({ error: "Not found" }),
    } as Response);

    await expect(apiClient.get("/documents/999")).rejects.toThrow("Not found");
  });

  it("should include status on ApiError", async () => {
    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: false,
      status: 500,
      json: async () => ({ error: "Server error" }),
    } as Response);

    try {
      await apiClient.get("/documents/1");
      expect.fail("Should have thrown");
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect((error as ApiError).status).toBe(500);
      expect((error as ApiError).isRetryable).toBe(true);
    }
  });

  it("should send POST body as JSON", async () => {
    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ success: true }),
    } as Response);

    await apiClient.post("/documents/1/generate", { density: "balanced" });

    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining("/documents/1/generate"),
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ density: "balanced" }),
      })
    );
  });

  it("should send PATCH request", async () => {
    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ id: "1", filename: "new.pdf" }),
    } as Response);

    await apiClient.patch("/documents/1", { filename: "new.pdf" });

    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining("/documents/1"),
      expect.objectContaining({
        method: "PATCH",
        body: JSON.stringify({ filename: "new.pdf" }),
      })
    );
  });

  it("should send DELETE request", async () => {
    vi.mocked(global.fetch).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ deleted: true }),
    } as Response);

    await apiClient.delete("/documents/1");

    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining("/documents/1"),
      expect.objectContaining({ method: "DELETE" })
    );
  });
});
