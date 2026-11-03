import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App } from "./App";
import { installAppLanguageSynchronization, synchronizeAppLanguage } from "./lib/appLocale";
import { installQueryLifecycle } from "./lib/queryLifecycle";
import { retryCompanionQuery } from "./lib/transport";
import "@volum/design/tokens.css";
import "@volum/design/primitives.css";
import "./styles.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 15_000, retry: retryCompanionQuery, refetchOnWindowFocus: true }
  }
});
installQueryLifecycle(queryClient);

async function bootstrap() {
  await synchronizeAppLanguage();
  installAppLanguageSynchronization();
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </StrictMode>
  );
}

void bootstrap();
