/** @vitest-environment jsdom */

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DatePicker } from "./DatePicker";

describe("DatePicker", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 8, 22, 12));
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("selects a date from the custom calendar", () => {
    const onChange = vi.fn();
    render(<DatePicker label="From" value="" onChange={onChange} />);

    fireEvent.click(screen.getByRole("button", { name: "Choose date" }));
    fireEvent.click(screen.getByRole("button", { name: /September 22, 2026/i }));

    expect(onChange).toHaveBeenCalledWith("2026-09-22");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("disables dates outside the allowed range and restores focus on Escape", () => {
    render(<DatePicker label="To" value="2026-09-22" min="2026-09-20" max="2026-09-24" onChange={() => {}} />);
    const trigger = screen.getByRole("button", { name: /Sep 22, 2026/i });

    fireEvent.click(trigger);
    expect((screen.getByRole("button", { name: /September 19, 2026/i }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: /September 25, 2026/i }) as HTMLButtonElement).disabled).toBe(true);

    fireEvent.keyDown(window, { key: "Escape" });
    vi.runAllTimers();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });
});
