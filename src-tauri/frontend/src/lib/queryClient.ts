import { QueryClient } from "@tanstack/react-query";

/** React Query 单例（模型目录分页等）。 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});
