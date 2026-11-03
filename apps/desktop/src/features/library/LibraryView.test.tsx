/** @vitest-environment jsdom */

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setAppLanguage } from "../../lib/i18n";
import type { LibraryRoot } from "../../types";
import { FirstLibraryState } from "./LibraryView";

const root = (overrides: Partial<LibraryRoot>): LibraryRoot => ({
  id: "root",
  path: "/Models",
  displayName: "Models",
  status: "online",
  lastScanAt: "2026-09-25T00:00:00Z",
  modelCount: 0,
  rootKind: "linked",
  ...overrides
});

describe("first library state", () => {
  beforeEach(() => setAppLanguage("en"));
  afterEach(cleanup);

  it("gives an empty managed library useful next actions", () => {
    const onAddFolder = vi.fn();
    const onNewProject = vi.fn();
    render(<FirstLibraryState roots={[root({ rootKind: "managed" })]} addingFolder={false} error="" onAddFolder={onAddFolder} onNewProject={onNewProject} />);

    expect(screen.getByText("Your Volum library is ready")).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "New project" }));
    fireEvent.click(screen.getByRole("button", { name: "Add existing folder" }));
    expect(onNewProject).toHaveBeenCalledOnce();
    expect(onAddFolder).toHaveBeenCalledOnce();
  });

  it("explains that the initial scan is still making progress", () => {
    render(<FirstLibraryState roots={[root({ status: "scanning", lastScanAt: undefined })]} addingFolder={false} error="" onAddFolder={vi.fn()} onNewProject={vi.fn()} />);

    expect(screen.getByText("Finding your 3D projects…")).not.toBeNull();
    expect(screen.queryByText("No supported 3D files found")).toBeNull();
  });

  it("distinguishes a completed empty scan from loading", () => {
    render(<FirstLibraryState roots={[root({})]} addingFolder={false} error="" onAddFolder={vi.fn()} onNewProject={vi.fn()} />);

    expect(screen.getByText("No supported 3D files found")).not.toBeNull();
    expect(screen.getByRole("button", { name: "Choose another folder" })).not.toBeNull();
  });
});
