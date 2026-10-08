import { useCallback, useEffect, useRef, useState } from "react";
import { onFetchDone } from "./data";

/** 取得が続けて終わったときに、読み直しをまとめる間隔 */
const RELOAD_DEBOUNCE_MS = 500;

export type Loaded<T> = {
  data: T | undefined;
  error: string | null;
  reload: () => void;
};

/**
 * `load` で読んだ値を返す。`deps` が変わったときと、取得が終わったとき（fetch-done）に読み直す
 */
export function useLoad<T>(load: () => Promise<T>, deps: unknown[]): Loaded<T> {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<string | null>(null);
  const loadRef = useRef(load);
  loadRef.current = load;

  const reload = useCallback(() => {
    loadRef.current().then(
      (value) => {
        setData(value);
        setError(null);
      },
      (err: unknown) => setError(String(err)),
    );
  }, deps);

  useEffect(reload, [reload]);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unlisten = onFetchDone(() => {
      clearTimeout(timer);
      timer = setTimeout(reload, RELOAD_DEBOUNCE_MS);
    });
    return () => {
      clearTimeout(timer);
      void unlisten.then((stop) => stop(), () => undefined);
    };
  }, [reload]);

  return { data, error, reload };
}
