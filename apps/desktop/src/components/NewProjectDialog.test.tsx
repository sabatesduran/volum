/** @vitest-environment jsdom */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { NewProjectDialog } from "./NewProjectDialog";

const { createProject } = vi.hoisted(() => ({
  createProject: vi.fn(async () => "linked-project")
}));

vi.mock("../lib/tauri/api", () => ({
  isTauri: () => false,
  api: {
    managedLibrary: async () => ({ root: null, projectsPath: null, inboxPath: null, trashPath: null }),
    libraryAssets: async () => [],
    configureManagedLibrary: vi.fn(),
    createProject
  }
}));

describe("new project storage choices", () => {
  beforeEach(() => createProject.mockClear());

  it("can keep a file selected from disk linked", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <NewProjectDialog onClose={vi.fn()} />
      </QueryClientProvider>
    );

    fireEvent.click(screen.getByRole("button", { name: "Choose files…" }));
    const keepLinked = screen.getByText("Keep linked").closest("button");
    await waitFor(() => expect(keepLinked?.disabled).toBe(false));
    fireEvent.click(keepLinked!);
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));

    await waitFor(() => expect(createProject).toHaveBeenCalledWith({
      name: "model",
      filePaths: ["/Users/you/Downloads/model.stl"],
      assetIds: [],
      transferMode: "link"
    }));
  });
});
