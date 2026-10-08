/**
 * Committed ADRs (docs/adr/*.md present in HEAD) are immutable except for the status line.
 *
 *   adr-guard.ts pre-commit          git pre-commit hook: reject staged body changes / deletions
 *   adr-guard.ts claude-pre-tool-use Claude Code PreToolUse (Edit|Write|MultiEdit): block before writing
 *   adr-guard.ts claude-post-bash    Claude Code PostToolUse (Bash): report working-tree violations
 */

const STATUS_LINE = /^ステータス[:：]/;

export function isAdrPath(relPath: string): boolean {
  return /^docs\/adr\/[^/]+\.md$/.test(relPath);
}

/** Replace every status line with a placeholder so only the rest of the text is compared. */
export function normalize(text: string): string {
  return text
    .split('\n')
    .map((line) => (STATUS_LINE.test(line) ? 'ステータス:' : line))
    .join('\n');
}

export function onlyStatusLineChanged(before: string, after: string): boolean {
  return normalize(before) === normalize(after);
}

interface EditOp {
  old_string: string;
  new_string: string;
  replace_all?: boolean;
}

function applyEdit(text: string, op: EditOp): string | undefined {
  if (!text.includes(op.old_string)) return undefined;
  return op.replace_all
    ? text.split(op.old_string).join(op.new_string)
    : text.replace(op.old_string, () => op.new_string);
}

/**
 * The file content a Claude Code file tool would produce, or undefined when it cannot be
 * computed (the tool call would fail on its own, e.g. old_string not found).
 */
export function resultOfToolCall(
  toolName: string,
  input: Record<string, unknown>,
  current: string,
): string | undefined {
  switch (toolName) {
    case 'Write':
      return typeof input.content === 'string' ? input.content : undefined;
    case 'Edit':
      return applyEdit(current, input as unknown as EditOp);
    case 'MultiEdit': {
      let text: string | undefined = current;
      for (const op of (input.edits as EditOp[] | undefined) ?? []) {
        if (text === undefined) return undefined;
        text = applyEdit(text, op);
      }
      return text;
    }
    default:
      return undefined;
  }
}

async function git(cwd: string, ...args: string[]): Promise<{ ok: boolean; out: string }> {
  const { success, stdout } = await new Deno.Command('git', {
    args,
    cwd,
    stdout: 'piped',
    stderr: 'null',
  }).output();
  return { ok: success, out: new TextDecoder().decode(stdout) };
}

async function repoRoot(dir: string): Promise<string | undefined> {
  const r = await git(dir, 'rev-parse', '--show-toplevel');
  return r.ok ? r.out.trim() : undefined;
}

async function headContent(root: string, relPath: string): Promise<string | undefined> {
  const r = await git(root, 'show', `HEAD:${relPath}`);
  return r.ok ? r.out : undefined;
}

async function readText(path: string): Promise<string | undefined> {
  try {
    return await Deno.readTextFile(path);
  } catch {
    return undefined;
  }
}

const RULE =
  'docs/adr のコミット済み ADR は変更できません（ステータス行のみ変更可）。決定を変えるときは新しい ADR を追加し、元の ADR のステータス行だけを更新してください（ADR 0002）。';

async function preCommit(): Promise<number> {
  const root = await repoRoot(Deno.cwd());
  if (!root) return 0;
  if (!(await git(root, 'rev-parse', '--verify', '-q', 'HEAD')).ok) return 0;

  const diff = await git(root, 'diff', '--cached', '--name-status', '--no-renames', 'HEAD', '--', 'docs/adr');
  const violations: string[] = [];
  for (const line of diff.out.split('\n').filter(Boolean)) {
    const [status, relPath] = line.split('\t');
    if (!status || !relPath || !isAdrPath(relPath)) continue;
    const before = await headContent(root, relPath);
    if (before === undefined) continue;
    if (status === 'D') {
      violations.push(`${relPath}: 削除`);
      continue;
    }
    const staged = await git(root, 'show', `:${relPath}`);
    if (!staged.ok || !onlyStatusLineChanged(before, staged.out)) {
      violations.push(`${relPath}: ステータス行以外の変更`);
    }
  }
  if (violations.length === 0) return 0;
  console.error(`pre-commit: ${RULE}\n${violations.map((v) => `  - ${v}`).join('\n')}`);
  return 1;
}

async function readHookInput(): Promise<Record<string, unknown>> {
  const raw = await new Response(Deno.stdin.readable).text();
  return raw.trim() ? JSON.parse(raw) : {};
}

async function claudePreToolUse(): Promise<number> {
  const payload = await readHookInput();
  const toolName = String(payload.tool_name ?? '');
  const input = (payload.tool_input ?? {}) as Record<string, unknown>;
  const filePath = typeof input.file_path === 'string' ? input.file_path : undefined;
  if (!filePath) return 0;

  const dir = filePath.slice(0, filePath.lastIndexOf('/')) || '/';
  const root = await repoRoot(dir).catch(() => undefined);
  if (!root || !filePath.startsWith(`${root}/`)) return 0;
  const relPath = filePath.slice(root.length + 1);
  if (!isAdrPath(relPath)) return 0;

  const before = await headContent(root, relPath);
  if (before === undefined) return 0;

  const current = (await readText(filePath)) ?? before;
  const after = resultOfToolCall(toolName, input, current);
  if (after === undefined) return 0;
  if (onlyStatusLineChanged(before, after)) return 0;

  console.error(`${relPath}: ${RULE}`);
  return 2;
}

async function claudePostBash(): Promise<number> {
  const payload = await readHookInput();
  const start = Deno.env.get('CLAUDE_PROJECT_DIR') ?? String(payload.cwd ?? Deno.cwd());
  const root = await repoRoot(start).catch(() => undefined);
  if (!root) return 0;

  const tracked = await git(root, 'ls-tree', '-r', '--name-only', 'HEAD', '--', 'docs/adr');
  if (!tracked.ok) return 0;
  const violations: string[] = [];
  for (const relPath of tracked.out.split('\n').filter(isAdrPath)) {
    const before = await headContent(root, relPath);
    if (before === undefined) continue;
    const now = await readText(`${root}/${relPath}`);
    if (now === undefined) violations.push(`${relPath}: 削除された`);
    else if (!onlyStatusLineChanged(before, now)) violations.push(`${relPath}: ステータス行以外が変更された`);
  }
  if (violations.length === 0) return 0;
  console.error(
    `直前のコマンドがコミット済み ADR を変更しました。${RULE}\n` +
      `${violations.map((v) => `  - ${v}`).join('\n')}\n` +
      '`git checkout HEAD -- <path>` で元に戻し、必要ならステータス行だけを変更し直してください。',
  );
  return 2;
}

if (import.meta.main) {
  const mode = Deno.args[0];
  const run = mode === 'pre-commit'
    ? preCommit
    : mode === 'claude-pre-tool-use'
    ? claudePreToolUse
    : mode === 'claude-post-bash'
    ? claudePostBash
    : undefined;
  if (!run) {
    console.error('usage: adr-guard.ts pre-commit | claude-pre-tool-use | claude-post-bash');
    Deno.exit(64);
  }
  Deno.exit(await run());
}
