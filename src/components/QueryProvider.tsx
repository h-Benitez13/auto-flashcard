"use client";

/**
 * QueryClientProvider wrapper
 *
 * Why this exists:
 * - QueryClient is a class instance and cannot be created in a Server Component
 * - Next.js cannot serialize class instances across server/client boundary
 * - We create it here, inside a Client Component, and pass it to the provider
 *
 * Usage: wrap the app in layout.tsx with <QueryProvider>{children}</QueryProvider>
 */

import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 30 * 1000,
        gcTime: 5 * 60 * 1000,
        retry: 2,
        retryDelay: (attemptIndex) =>
          Math.min(1000 * Math.pow(2, attemptIndex), 30000),
        refetchOnReconnect: false,
        refetchOnWindowFocus: false,
        refetchOnMount: false,
      },
      mutations: {
        retry: 1,
        retryDelay: (attemptIndex) =>
          Math.min(1000 * Math.pow(2, attemptIndex), 10000),
      },
    },
  });
}

export function QueryProvider({ children }: { children: React.ReactNode }) {
  // Create QueryClient once per client session
  const [queryClient] = useState(() => createQueryClient());

  return (
    <QueryClientProvider client={queryClient}>
      {children}
    </QueryClientProvider>
  );
}
