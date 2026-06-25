import { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

/**
 * Test utility: create a fresh QueryClient for each test
 *
 * Important: each test gets its own QueryClient to prevent
 * cache pollution between tests.
 */
export function createTestQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: Infinity,
        gcTime: Infinity,
      },
      mutations: {
        retry: false,
      },
    },
  });
}

export function QueryClientWrapper({
  children,
  client,
}: {
  children: ReactNode;
  client?: QueryClient;
}) {
  const queryClient = client ?? createTestQueryClient();
  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}
