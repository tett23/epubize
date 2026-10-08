import { useNormalizeOptions, type NormalizeOptions } from "../normalizeOptions";

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

function Checkbox({
  novelId,
  name,
  options,
  update,
}: {
  novelId: number;
  name: BooleanKey;
  options: NormalizeOptions;
  update: ReturnType<typeof useNormalizeOptions>[1];
}) {
  const id = `${name}-${novelId}`;
  return (
    <label htmlFor={id} className="inline-flex items-center gap-1.5">
      {name}
      <input id={id} type="checkbox" checked={options[name]} onChange={(e) => update(name, e.target.checked)} />
    </label>
  );
}

export function NormalizeOptionsForm({ novelId }: { novelId: number }) {
  const [options, update] = useNormalizeOptions(novelId);
  const props = { novelId, options, update };

  return (
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
      {booleanKeys.map((name) => (
        <Checkbox key={name} name={name} {...props} />
      ))}
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
      <Checkbox name="joinVoicedAndSemiVoicedMark" {...props} />
    </fieldset>
  );
}
