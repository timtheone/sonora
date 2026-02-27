import { describe, expect, it } from "vitest";
import { appendInsertionRecord, type InsertionRecord } from "./insertion-history";

describe("insertion history", () => {
  it("prepends new insertion records", () => {
    const existing: InsertionRecord[] = [{ text: "older", status: "success" }];
    const updated = appendInsertionRecord(existing, {
      text: "new",
      status: "fallback",
    });

    expect(updated.map((record) => record.text)).toEqual(["new", "older"]);
  });

  it("keeps only the latest 3 records", () => {
    const existing: InsertionRecord[] = [
      { text: "one", status: "success" },
      { text: "two", status: "success" },
      { text: "three", status: "success" },
    ];

    const updated = appendInsertionRecord(existing, {
      text: "four",
      status: "fallback",
    });

    expect(updated).toHaveLength(3);
    expect(updated.map((record) => record.text)).toEqual(["four", "one", "two"]);
  });
});
