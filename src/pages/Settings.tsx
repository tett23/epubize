import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import { ExternalLink } from "../components/ExternalLink";
import { novelPath } from "../components/links";
import { ActionButton, Button, InternalLink } from "../components/ui";
import {
  type EnvCheck,
  getSettings,
  listNovels,
  listSubscriptions,
  removeSubscription,
  type SettingsView,
  saveSettings,
} from "../data";
import { useLoad } from "../hooks";
import { type NovelSummary, uniqueId } from "../models";
import { LoadError } from "./Root";

/** 管理画面（ADR 0020） */
export function Settings() {
  const view = useLoad(getSettings, []);
  const subscriptions = useLoad(listSubscriptions, []);
  const novels = useLoad(listNovels, []);
  // 購読と、取得済みの作品を対応させ、作品名を出す
  const novelsById = new Map((novels.data ?? []).map((novel) => [uniqueId(novel), novel]));

  return (
    <div className="max-w-4xl space-y-8">
      <h2 className="text-xl font-bold">settings</h2>
      {view.error && <LoadError error={view.error} />}
      {view.data && <SettingsForm view={view.data} onSaved={view.reload} />}
      <Section title="購読">
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          購読から外しても、取得済みの作品のデータは消えません。外す前の一覧は novels.json.bak に残ります。
        </p>
        {subscriptions.error && <LoadError error={subscriptions.error} />}
        {subscriptions.data && (
          <table className="border-collapse">
            <tbody>
              {subscriptions.data.map((item) => (
                <tr key={uniqueId(item)} className="border-t border-neutral-200 dark:border-neutral-700">
                  <td className="py-1 pr-4 whitespace-nowrap">
                    <ExternalLink href={item.url}>{uniqueId(item)}</ExternalLink>
                  </td>
                  <td className="py-1 pr-4">
                    <NovelTitle novel={novelsById.get(uniqueId(item))} />
                  </td>
                  <td className="py-1">
                    <ActionButton
                      confirm={`${uniqueId(item)} を購読から外します。よろしいですか？`}
                      action={async () => {
                        await removeSubscription(item.site, item.siteId);
                        subscriptions.reload();
                      }}
                    >
                      remove
                    </ActionButton>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {subscriptions.data?.length === 0 && (
          <p className="text-sm text-neutral-600 dark:text-neutral-400">購読している作品はありません。</p>
        )}
      </Section>
      {view.data && <Info view={view.data} />}
    </div>
  );
}

function NovelTitle({ novel }: { novel: NovelSummary | undefined }) {
  if (novel == null) {
    return <span className="text-sm text-neutral-500 dark:text-neutral-400">（未取得）</span>;
  }
  return <InternalLink to={novelPath(novel.id)}>{novel.title}</InternalLink>;
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-3">
      <h3 className="border-b border-neutral-200 pb-1 text-lg font-bold dark:border-neutral-700">{title}</h3>
      {children}
    </section>
  );
}

type Message = { kind: "info" | "error"; text: string };

const inputClass =
  "rounded border border-neutral-300 px-2 py-1 dark:border-neutral-600 dark:bg-neutral-800 disabled:opacity-60";

function SettingsForm({ view, onSaved }: { view: SettingsView; onSaved: () => void }) {
  const [crawlerPath, setCrawlerPath] = useState(view.settings.crawlerPath ?? "");
  const [epubBuilderPath, setEpubBuilderPath] = useState(view.settings.epubBuilderPath ?? "");
  const [sendToKindlePath, setSendToKindlePath] = useState(view.settings.sendToKindlePath ?? "");
  const [envPath, setEnvPath] = useState(view.settings.sendToKindleEnvPath ?? "");
  const [envExamplePath, setEnvExamplePath] = useState(view.settings.sendToKindleEnvExamplePath ?? "");
  const [enabled, setEnabled] = useState(view.settings.schedule.enabled);
  const [at, setAt] = useState(view.settings.schedule.at);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<Message | null>(null);

  useEffect(() => {
    setCrawlerPath(view.settings.crawlerPath ?? "");
    setEpubBuilderPath(view.settings.epubBuilderPath ?? "");
    setSendToKindlePath(view.settings.sendToKindlePath ?? "");
    setEnvPath(view.settings.sendToKindleEnvPath ?? "");
    setEnvExamplePath(view.settings.sendToKindleEnvExamplePath ?? "");
    setEnabled(view.settings.schedule.enabled);
    setAt(view.settings.schedule.at);
  }, [view]);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await saveSettings({
        crawlerPath: crawlerPath.trim() === "" ? null : crawlerPath.trim(),
        epubBuilderPath: epubBuilderPath.trim() === "" ? null : epubBuilderPath.trim(),
        sendToKindlePath: sendToKindlePath.trim() === "" ? null : sendToKindlePath.trim(),
        sendToKindleEnvPath: envPath.trim() === "" ? null : envPath.trim(),
        sendToKindleEnvExamplePath: envExamplePath.trim() === "" ? null : envExamplePath.trim(),
        schedule: { enabled, at },
      });
      setMessage({ kind: "info", text: "保存しました" });
      onSaved();
    } catch (err) {
      setMessage({ kind: "error", text: String(err) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <form className="space-y-8" onSubmit={onSubmit}>
      {view.loadError && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">
          設定のファイルを読めないため、既定値を表示しています: {view.loadError}
        </p>
      )}
      <Section title="クローラー">
        <ExecutableField
          name="クローラー"
          value={crawlerPath}
          onChange={setCrawlerPath}
          inUse={view.crawlerInUse}
          found={view.crawlerFound}
        />
      </Section>
      <Section title="epub-builder">
        <ExecutableField
          name="epub-builder "
          value={epubBuilderPath}
          onChange={setEpubBuilderPath}
          inUse={view.epubBuilderInUse}
          found={view.epubBuilderFound}
        />
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          download epub と send to Kindle で EPUB 3.0 を作るときに、子プロセスとして起動します。
        </p>
      </Section>
      <Section title="send-to-kindle">
        <ExecutableField
          name="send-to-kindle "
          value={sendToKindlePath}
          onChange={setSendToKindlePath}
          inUse={view.sendToKindleInUse}
          found={view.sendToKindleFound}
        />
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          send to Kindle で EPUB を送るときに、子プロセスとして起動します。
        </p>
        <FileField
          name=".env"
          value={envPath}
          onChange={setEnvPath}
          inUse={view.settings.sendToKindleEnvPath ?? view.sendToKindleEnvDefault}
          defaultPath={view.sendToKindleEnvDefault}
        />
        <FileField
          name=".env.example"
          value={envExamplePath}
          onChange={setEnvExamplePath}
          inUse={view.settings.sendToKindleEnvExamplePath ?? view.sendToKindleEnvExampleDefault}
          defaultPath={view.sendToKindleEnvExampleDefault}
        />
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          空欄ならデータの置き場所のものを使います。.env は send-to-kindle に -e で渡します。.env.example
          があれば、そのキーが .env にあるかを調べます。値は表示しません。
        </p>
        <EnvCheckResult check={view.sendToKindleEnvCheck} />
      </Section>
      <Section title="定期取得">
        <label className="flex items-center gap-2">
          <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
          アプリが動いている間、毎日 fetch all を行う
        </label>
        <label className="flex items-center gap-2">
          時刻
          <input
            type="time"
            value={at}
            onChange={(e) => setAt(e.target.value)}
            disabled={!enabled}
            required
            className={inputClass}
          />
        </label>
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          時刻を過ぎたときにスリープしていた場合は、起きた後に行います。
        </p>
      </Section>
      <div className="flex items-center gap-3">
        <Button type="submit" disabled={saving}>
          保存
        </Button>
        {message && (
          <span
            role={message.kind === "error" ? "alert" : "status"}
            className={`text-sm ${message.kind === "error" ? "text-red-600 dark:text-red-400" : "text-neutral-600 dark:text-neutral-400"}`}
          >
            {message.text}
          </span>
        )}
      </div>
    </form>
  );
}

/** 実行ファイルの指定。空欄なら自動で探す。いま使っているものと、自動で見つかるものを出す */
function ExecutableField({
  name,
  value,
  onChange,
  inUse,
  found,
}: {
  name: string;
  value: string;
  onChange: (value: string) => void;
  inUse: string | null;
  found: string | null;
}) {
  return (
    <>
      <label className="block space-y-1">
        <span className="text-sm">実行ファイル</span>
        <input
          type="text"
          aria-label={`${name}の実行ファイル`}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={found ?? "空欄なら PATH などから自動で探します"}
          className={`w-full ${inputClass}`}
        />
      </label>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm text-neutral-600 dark:text-neutral-400">
        <dt>使用中</dt>
        <dd className="break-all">{inUse ?? "見つかりません"}</dd>
        <dt>自動で見つかるもの</dt>
        <dd className="break-all">{found ?? "見つかりません"}</dd>
      </dl>
    </>
  );
}

/** send-to-kindle の設定のファイルの指定。空欄なら既定の場所を使う。保存した設定で使うものを出す */
function FileField({
  name,
  value,
  onChange,
  inUse,
  defaultPath,
}: {
  name: string;
  value: string;
  onChange: (value: string) => void;
  inUse: string;
  defaultPath: string;
}) {
  return (
    <>
      <label className="block space-y-1">
        <span className="text-sm">{name}</span>
        <input
          type="text"
          aria-label={`send-to-kindle の ${name}`}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={defaultPath}
          className={`w-full ${inputClass}`}
        />
      </label>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm text-neutral-600 dark:text-neutral-400">
        <dt>使用中</dt>
        <dd className="break-all">{inUse}</dd>
      </dl>
    </>
  );
}

/** .env.example と比べた .env の状態。保存した設定について出す */
function EnvCheckResult({ check }: { check: EnvCheck | null }) {
  if (check == null) {
    return null;
  }
  const [ok, text] =
    check.kind === "complete"
      ? [true, ".env に .env.example のキーがすべてあります"]
      : check.kind === "missing"
        ? [false, `.env に足りないキー: ${check.keys.join(", ")}`]
        : [false, check.error];
  return (
    <p
      data-testid="env-check"
      className={`text-sm break-all ${ok ? "text-neutral-600 dark:text-neutral-400" : "text-red-600 dark:text-red-400"}`}
    >
      {text}
    </p>
  );
}

function Info({ view }: { view: SettingsView }) {
  const rows: [string, string][] = [
    ["環境", view.environment],
    ["購読の一覧", view.subscriptionsPath],
    ["設定", view.settingsPath],
    ["データベース", view.databasePath],
  ];
  return (
    <Section title="データの置き場所">
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        {rows.map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-neutral-600 dark:text-neutral-400">{label}</dt>
            <dd className="font-mono break-all">{value}</dd>
          </div>
        ))}
      </dl>
    </Section>
  );
}
