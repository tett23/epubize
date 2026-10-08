/** 整形の設定。項目は kindlize と同じ。作品ごとにデータベースに JSON で持つ（ADR 0017） */
export type NormalizeOptions = {
  direction: "vertical" | "horizontal";
  removeEmptyLine: boolean;
  replaceConsecutiveBreak: boolean;
  replaceParens: boolean;
  insertIndent: boolean;
  replacePunctuation: boolean;
  replaceHanZen: boolean;
  replaceSpaceHanZenAfterPunctuations: boolean;
  /** 句読点で始まる行に付けるクラス名。空なら付けない */
  indentPunctuationLine: string;
  joinVoicedAndSemiVoicedMark: boolean;
};

export const defaultNormalizeOptions: NormalizeOptions = {
  direction: "vertical",
  removeEmptyLine: true,
  replaceConsecutiveBreak: true,
  replaceParens: true,
  insertIndent: true,
  replacePunctuation: true,
  replaceHanZen: true,
  replaceSpaceHanZenAfterPunctuations: true,
  indentPunctuationLine: "ih",
  joinVoicedAndSemiVoicedMark: true,
};

/** データベースに持つ値を、既定に重ねて読む。型の合わない項目は既定のままにする */
export function resolveNormalizeOptions(stored: Record<string, unknown> | null): NormalizeOptions {
  const options: NormalizeOptions = { ...defaultNormalizeOptions };
  if (stored == null) {
    return options;
  }
  for (const key of Object.keys(defaultNormalizeOptions) as (keyof NormalizeOptions)[]) {
    const value = stored[key];
    if (key === "direction") {
      if (value === "vertical" || value === "horizontal") {
        options.direction = value;
      }
    } else if (typeof value === typeof defaultNormalizeOptions[key]) {
      (options as Record<string, unknown>)[key] = value;
    }
  }
  return options;
}
