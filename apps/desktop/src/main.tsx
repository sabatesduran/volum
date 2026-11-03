import { lazy, StrictMode, Suspense } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./app/App";
import { initializeDocumentLanguage } from "./lib/i18n";
import "@volum/design/tokens.css";
import "@volum/design/primitives.css";
import "./styles/global.css";

initializeDocumentLanguage();

const UpdateManager = lazy(() => import("./features/updates/UpdateManager").then((module) => ({ default: module.UpdateManager })));

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 10_000, retry: 1, refetchOnWindowFocus: false }
  }
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
      <Suspense fallback={null}><UpdateManager /></Suspense>
    </QueryClientProvider>
  </StrictMode>
);
