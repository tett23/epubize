import { createContext, useContext, useState, type ReactNode } from "react";

/** 整形の設定。項目は kindlize と同じ */
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

type Store = {
  options: Record<number, NormalizeOptions>;
  set: (novelId: number, options: NormalizeOptions) => void;
};

const NormalizeOptionsContext = createContext<Store | null>(null);

/** 作品ごとの整形の設定を持つ。いまはメモリ上だけに持ち、再起動で既定に戻る */
export function NormalizeOptionsProvider({ children }: { children: ReactNode }) {
  const [options, setOptions] = useState<Record<number, NormalizeOptions>>({});
  const set = (novelId: number, value: NormalizeOptions) =>
    setOptions((prev) => ({ ...prev, [novelId]: value }));

  return <NormalizeOptionsContext value={{ options, set }}>{children}</NormalizeOptionsContext>;
}

export function useNormalizeOptions(novelId: number) {
  const store = useContext(NormalizeOptionsContext);
  if (store == null) {
    throw new Error("NormalizeOptionsProvider is missing");
  }
  const options = store.options[novelId] ?? defaultNormalizeOptions;
  const update = <K extends keyof NormalizeOptions>(key: K, value: NormalizeOptions[K]) =>
    store.set(novelId, { ...options, [key]: value });

  return [options, update] as const;
}
