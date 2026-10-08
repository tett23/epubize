# ADR 0002: コミット済みの ADR を変更不可とし、hook で強制する

ステータス: 採択

## 文脈

ADR は決定とその理由の記録であり、後から本文を書き換えると、いつ何を根拠に決めたのかが分からなくなる。
CLAUDE.md に規則を書くだけでは守られないことがあるため、仕組みで縛る。

このリポジトリでは、まだアプリケーションの実行環境（言語）を決めていない。

## 実装すること

- `docs/adr/*.md` のうち HEAD に存在するもの（コミット済みの ADR）は、本文の変更も削除もできないものとする
- 例外は `ステータス:` で始まる行だけとする。この行は書き換えてよい。行の追加、削除、移動は例外に含めない
- 判定は `scripts/adr-guard.ts` に一つだけ置き、次の三つの入口から呼ぶ
  - Claude Code の PreToolUse hook（Edit / Write / MultiEdit）：適用後の内容を計算し、本文が変わるなら実行前に止める
  - Claude Code の PostToolUse hook（Bash）：実行後に作業ツリーのコミット済み ADR を HEAD と比べ、違反があれば Claude に差し戻す
  - git の pre-commit hook：コミットされる内容を HEAD と比べ、本文の変更と削除を拒否する
- Claude Code の hook は `.claude/settings.json` に、git の hook は `.githooks/pre-commit` に置く。clone した後は `git config core.hooksPath .githooks` で有効にする
- 判定スクリプトは Deno で書き、外部の依存を持たず、`deno.json` なしで（`--no-config`）実行する。権限は読み取りと `git` の実行だけとする

## 実装しないこと

- `git commit --no-verify` や `git rebase` などによる履歴の書き換えは防げない
- サーバ側（push 時）の検査はしない
- この hook のために、アプリケーションの実行環境を Deno に決めることはしない。実行環境は別の ADR で決める

## テスト設計

- `test/adr-guard.test.ts` で、ADR のパスの判定、ステータス行だけの変更の許可、本文の変更・ステータス行の追加と削除の拒否、Edit / MultiEdit / Write の適用後の内容の計算をテストする
- 各 hook に実際の入力を流して確認した
  - PreToolUse：コミット済み ADR の本文の Edit を止め、ステータス行だけの Edit と未コミットの ADR を通すこと
  - PostToolUse（Bash）と pre-commit：ステータス行だけの変更を通し、本文の変更を拒否すること

## トレードオフ

- hook を動かすために、開発環境に Deno が必要になる
- Bash による変更は、実行を止めるのではなく実行後に検出する
- 誤字の修正のような小さな変更も、ステータス行以外は許されない
- `core.hooksPath` は clone ごとに設定が要る
