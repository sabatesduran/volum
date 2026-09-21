import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./app/App";
import { initializeDocumentLanguage } from "./lib/i18n";
import { UpdateManager } from "./features/updates/UpdateManager";
import "./styles/global.css";

initializeDocumentLanguage();

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 10_000, retry: 1, refetchOnWindowFocus: false }
  }
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
      <UpdateManager />
    </QueryClientProvider>
  </StrictMode>
);
