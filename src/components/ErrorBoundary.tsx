"use client";

/**
 * Global error boundary + toast error handler
 *
 * This component:
 * 1. Catches unhandled promise rejections
 * 2. Shows user-friendly error messages
 * 3. Logs to console for debugging
 */

import { ReactNode, useEffect } from "react";
import { toast } from "sonner";

interface Props {
  children: ReactNode;
}

export function ErrorBoundary({ children }: Props) {
  useEffect(() => {
    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      console.error("Unhandled promise rejection:", event.reason);

      const message =
        event.reason instanceof Error
          ? event.reason.message
          : "Something went wrong";

      toast.error(message);
    };

    window.addEventListener(
      "unhandledrejection",
      handleUnhandledRejection
    );

    return () => {
      window.removeEventListener(
        "unhandledrejection",
        handleUnhandledRejection
      );
    };
  }, []);

  return <>{children}</>;
}
