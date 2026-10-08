import { describe, expect, it } from "vitest";
import { formatDate, formatDateTime } from "./format";

// 書式はローカル時刻で決まるため、ローカル時刻で作った日時で確かめる
const local = (s: string) => new Date(s).toISOString();

describe("formatDate", () => {
  it("YYYY-MM-DD にし、月と日を 2 桁にする", () => {
    expect(formatDate(local("2026-03-04T12:00:00"))).toBe("2026-03-04");
    expect(formatDate(local("2026-12-31T23:59:59"))).toBe("2026-12-31");
  });
});

describe("formatDateTime", () => {
  it("YYYY-MM-DD HH:mm にし、24 時間制で 2 桁にする", () => {
    expect(formatDateTime(local("2026-03-04T05:06:00"))).toBe("2026-03-04 05:06");
    expect(formatDateTime(local("2026-03-04T21:00:00"))).toBe("2026-03-04 21:00");
    expect(formatDateTime(local("2026-03-04T00:00:00"))).toBe("2026-03-04 00:00");
  });
});
