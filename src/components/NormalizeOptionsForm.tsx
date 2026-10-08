import { useEffect, useState } from "react";
import { setNormalizeOptions } from "../data";
import { resolveNormalizeOptions, type NormalizeOptions } from "../normalizeOptions";

type BooleanKey = {
  [K in keyof NormalizeOptions]: NormalizeOptions[K] extends boolean ? K : never;
}[keyof NormalizeOptions];

const booleanKeys: BooleanKey[] = [
  "removeEmptyLine",
  "replaceConsecutiveBreak",
  "replaceParens",
  "insertIndent",
  "replacePunctuation",
  "replaceHanZen",
  "replaceSpaceHanZenAfterPunctuations",
];

/**
 * 作品の整形の設定。データベースの値を既定に重ねて読み、変えるたびに保存する（ADR 0017）。
 * 保存を待たずに画面へ反映する
 */
export function useNormalizeOptions(novelId: number, stored: Record<string, unknown> | null) {
  const [options, setOptions] = useState(() => resolveNormalizeOptions(stored));
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setOptions(resolveNormalizeOptions(stored));
  }, [novelId, stored]);

  const update = <K extends keyof NormalizeOptions>(key: K, value: NormalizeOptions[K]) => {
    const next = { ...options, [key]: value };
    setOptions(next);
    setNormalizeOptions(novelId, next).then(
      () => setError(null),
      (err: unknown) => setError(String(err)),
    );
  };

  return { options, update, error };
}

export function NormalizeOptionsForm({
  novelId,
  options,
  update,
  error,
}: { novelId: number } & ReturnType<typeof useNormalizeOptions>) {
  const checkbox = (name: BooleanKey) => {
    const id = `${name}-${novelId}`;
    return (
      <label key={name} htmlFor={id} className="inline-flex items-center gap-1.5">
        {name}
        <input id={id} type="checkbox" checked={options[name]} onChange={(e) => update(name, e.target.checked)} />
      </label>
    );
  };

  return (
    <div className="space-y-1">
      <fieldset className="flex flex-wrap items-center gap-x-5 gap-y-2 text-sm">
        <span className="inline-flex items-center gap-3">
          direction:
          {(["vertical", "horizontal"] as const).map((direction) => (
            <label key={direction} className="inline-flex items-center gap-1">
              <input
                type="radio"
                name={`direction-${novelId}`}
                checked={options.direction === direction}
                onChange={() => update("direction", direction)}
              />
              {direction}
            </label>
          ))}
        </span>
        {booleanKeys.map(checkbox)}
        <label htmlFor={`indentPunctuationLine-${novelId}`} className="inline-flex items-center gap-1.5">
          indentPunctuationLine
          <input
            id={`indentPunctuationLine-${novelId}`}
            type="text"
            value={options.indentPunctuationLine}
            onChange={(e) => update("indentPunctuationLine", e.target.value)}
            className="w-24 rounded border border-neutral-300 px-1.5 py-0.5 dark:border-neutral-600 dark:bg-neutral-800"
          />
        </label>
        {checkbox("joinVoicedAndSemiVoicedMark")}
      </fieldset>
      {error && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">
          設定を保存できません: {error}
        </p>
      )}
    </div>
  );
}
