import { describe, expect, it } from "vitest";
import { validatedExternalUrl } from "./external";

describe("validatedExternalUrl", () => {
  it("accepts ordinary public source links", () => {
    expect(validatedExternalUrl("https://www.printables.com/model/42").toString())
      .toBe("https://www.printables.com/model/42");
  });

  it.each([
    "javascript:alert(1)",
    "file:///private/library.db",
    "https://user:secret@example.com/model",
    "not a URL",
  ])("rejects unsafe source URL %s", (value) => {
    expect(() => validatedExternalUrl(value)).toThrow();
  });
});
