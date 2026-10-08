import { describe, expect, it } from "vitest";
import { defaultNormalizeOptions, resolveNormalizeOptions } from "./normalizeOptions";

describe("resolveNormalizeOptions", () => {
  it("保存した値がなければ既定値", () => {
    expect(resolveNormalizeOptions(null)).toEqual(defaultNormalizeOptions);
  });

  it("保存した値を既定値に重ねる", () => {
    expect(
      resolveNormalizeOptions({ direction: "horizontal", insertIndent: false, indentPunctuationLine: "" }),
    ).toEqual({
      ...defaultNormalizeOptions,
      direction: "horizontal",
      insertIndent: false,
      indentPunctuationLine: "",
    });
  });

  it("型の合わない値と知らない項目は捨てる", () => {
    expect(
      resolveNormalizeOptions({
        direction: "diagonal",
        removeEmptyLine: "yes",
        indentPunctuationLine: 1,
        unknown: true,
      }),
    ).toEqual(defaultNormalizeOptions);
  });

  it("既定値を書き換えない", () => {
    const options = resolveNormalizeOptions({ insertIndent: false });
    options.removeEmptyLine = false;
    expect(defaultNormalizeOptions.removeEmptyLine).toBe(true);
    expect(defaultNormalizeOptions.insertIndent).toBe(true);
  });
});
