/**
 * HTTP client with AbortController + retry logic
 *
 * This layer handles:
 * - Automatic request cancellation (race condition prevention)
 * - Retry on network failures
 * - Error mapping to typed errors
 * - Request timeout (30s)
 *
 * Key difference vs raw fetch:
 * - Raw fetch: multiple requests to same endpoint -> multiple responses
 * - ApiClient: auto-abort previous request if new one comes in
 */

const API_URL =
  process.env.NEXT_PUBLIC_API_URL || "http://localhost:3001";

interface ApiErrorResponse {
  error?: string;
  message?: string;
  status?: number;
}

export class ApiError extends Error {
  status: number;
  isRetryable: boolean;

  constructor(
    message: string,
    status: number = 500,
    isRetryable: boolean = status >= 500 || status === 408
  ) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.isRetryable = isRetryable;
  }
}

class ApiClient {
  private abortControllers = new Map<string, AbortController>();

  /**
   * Send a GET request
   * @param endpoint - e.g. "/documents" or "/documents/123"
   */
  async get<T>(endpoint: string): Promise<T> {
    return this.request<T>("GET", endpoint);
  }

  /**
   * Send a POST request
   * @param endpoint - e.g. "/documents/123/generate"
   * @param body - Request body (will be JSON-stringified)
   */
  async post<T>(endpoint: string, body?: unknown): Promise<T> {
    return this.request<T>("POST", endpoint, body);
  }

  /**
   * Send a PATCH request
   * @param endpoint - e.g. "/documents/123"
   * @param body - Request body with fields to update
   */
  async patch<T>(endpoint: string, body?: unknown): Promise<T> {
    return this.request<T>("PATCH", endpoint, body);
  }

  /**
   * Send a DELETE request
   * @param endpoint - e.g. "/documents/123"
   */
  async delete<T>(endpoint: string): Promise<T> {
    return this.request<T>("DELETE", endpoint);
  }

  /**
   * Core request method
   * - Cancels previous request to this endpoint (prevents race conditions)
   * - Sets 30s timeout
   * - Maps errors to typed ApiError
   */
  private async request<T>(
    method: string,
    endpoint: string,
    body?: unknown
  ): Promise<T> {
    // Cancel previous request to this endpoint if it exists
    // This prevents: nav to doc A -> nav to doc B -> response from A arrives late and overwrites B
    const abortKey = `${method}:${endpoint}`;
    this.abortControllers.get(abortKey)?.abort();

    // Create new abort controller for this request
    const controller = new AbortController();
    this.abortControllers.set(abortKey, controller);

    // Set 30s timeout
    const timeoutId = setTimeout(
      () => controller.abort(),
      30 * 1000
    );

    try {
      const response = await fetch(`${API_URL}${endpoint}`, {
        method,
        headers: {
          "Content-Type": "application/json",
        },
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        // Try to parse error details from API response
        let errorData: ApiErrorResponse = {};
        try {
          errorData = await response.json();
        } catch {
          // Response wasn't JSON, that's ok
        }

        const errorMessage =
          errorData.error ||
          errorData.message ||
          `HTTP ${response.status}`;

        throw new ApiError(errorMessage, response.status);
      }

      return response.json() as Promise<T>;
    } catch (error) {
      // Handle abort (timeout or user cancelled)
      if (error instanceof DOMException && error.name === "AbortError") {
        throw new ApiError(
          "Request timeout or cancelled",
          408,
          true
        );
      }

      // If it's already an ApiError, re-throw
      if (error instanceof ApiError) {
        throw error;
      }

      // Network error (no internet, CORS, etc)
      throw new ApiError(
        error instanceof Error ? error.message : "Network error",
        0,
        true
      );
    }
  }
}

export const apiClient = new ApiClient();
