export type InsertionStatus = "success" | "fallback" | "failure";

export interface InsertionRecord {
  text: string;
  status: InsertionStatus;
}

export function appendInsertionRecord(
  current: InsertionRecord[],
  next: InsertionRecord,
  limit = 3,
): InsertionRecord[] {
  return [next, ...current].slice(0, limit);
}
