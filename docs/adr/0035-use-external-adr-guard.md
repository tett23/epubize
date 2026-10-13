# ADR 0035: コミット済みの ADR の検査を、外部の adr-guard に置き換え、CI でも検査する

ステータス: 採択→[ADR 0036](0036-adr-guard-v0-8-0.md)

## 文脈

[ADR 0002](0002-immutable-adrs.md) で、コミット済みの ADR の変更を拒否する判定を `scripts/adr-guard.ts`（Deno）に置き、Claude Code の hook と git の pre-commit hook から呼ぶことにした。
CI では、このスクリプトのテストだけを動かし、ADR の変更そのものは検査しないとした（[ADR 0022](0022-development-infrastructure.md)）。

同じ規則を強制する道具は、独立したリポジトリの [adr-guard](https://github.com/tett23/adr-guard)（Rust）として作られ、tett23 の他のリポジトリ（novel-crawler、epub-builder、send-to-kindle など）はこれを使っている。
adr-guard には、このリポジトリのスクリプトにない次の機能がある。

- CI 用の composite action。push と pull request の範囲のコミットを一つずつ検査する（`check-range`）
- 文書（`.md`）の中の ADR の参照（`ADR 0001` の形）に、ADR のファイルへのリンクを求める検査（`links`）と、その修正（`links --fix`）
- ステータスの行の変更（`--allow-status`）と、ADR の参照のリンクだけの変更（`--allow-links`）を、オプションで許可すること

このリポジトリの文書には、リンクのない ADR の参照が多く残っている（`adr-guard links` で 300 か所ほど）。
また、Claude Code の adr-guard の hook（`adr-guard claude-hook --allow-status`）は、利用者の全体の設定（`~/.claude/settings.json`）で有効にしている。

手元の adr-guard は v0.5.0 である。

## 実装すること

- `.githooks/pre-commit` を `exec adr-guard pre-commit --allow-status --allow-links` にする
- `scripts/adr-guard.ts` と `test/adr-guard.test.ts` を消す
- `.claude/settings.json` から、`scripts/adr-guard.ts` を呼ぶ hook（PreToolUse と PostToolUse）を消す。ほかの設定がなくなるため、ファイルごと消す
  - Claude Code の hook は、全体の設定にある `adr-guard claude-hook --allow-status` に任せる
- CI（`.github/workflows/ci.yml`）の `adr-guard` ジョブ（`deno test`）を、adr-guard の composite action を使う `adr` ジョブに置き換える
  - `actions/checkout@v7` を `fetch-depth: 0`、`persist-credentials: false` で使う
  - `uses: tett23/adr-guard@v0.5.0` とし、`allow-status: true` と `allow-links: true` を付ける。範囲は、pull request ではベースの先端から pull request の先端まで、`main` への push では push 前の先端から push 後の先端までとする
  - action は `check-range` の後に `links` も実行する
- 文書の ADR の参照を、`adr-guard links --fix --include-committed` でリンクにする。コミット済みの ADR も対象にする
  - この書き換えは、ほかの変更と分けて一つのコミットにする
- ステータスの書き方を、後の ADR へのリンクを付けた `採択→[ADR NNNN](NNNN-xxx.md)` とする。既存の `採択→ADR NNNN` は、上の `links --fix` でこの形になる
- [ADR 0002](0002-immutable-adrs.md)、[ADR 0006](0006-react-typescript-pnpm-frontend.md)、[ADR 0022](0022-development-infrastructure.md) のステータスを `採択→[ADR 0035](0035-use-external-adr-guard.md)` にする。変えた内容は `docs/specifications.md` の「ADR の状態」の表に書く
  - 0002：判定を `scripts/adr-guard.ts` に置くこと、Bash の実行後の検査、Claude Code の hook をリポジトリに置くこと、ステータス行以外の変更を一切許さないこと（ADR の参照のリンクだけの変更を許す）
  - 0006：hook のために開発環境に Deno が要ること
  - 0022：CI の ADR guard のジョブの中身と、ADR の変更を CI で検査しないこと
- README の「開発」に、adr-guard のインストール（`cargo install --git https://github.com/tett23/adr-guard --tag v0.5.0`、またはリリースのバイナリ）と、hook の有効化を書く。Deno は hook のためには要らなくなる
- `docs/specifications.md` の「開発」と「ADR の状態」を、上の内容に合わせる

## 実装しないこと

- Claude Code の hook をリポジトリの `.claude/settings.json` に置くこと。全体の設定で有効にしているため
- Bash による ADR の書き換えを、実行の後に検出すること。adr-guard にこの機能はない。pre-commit hook と CI で検査する
- adr-guard を pnpm や mise で入れること。インストールは adr-guard の README の手順に従う
- epub-builder を動かすための Deno の要件の変更（[ADR 0029](0029-export-epub-zip-and-send-to-kindle.md)）。これは開発ではなく利用の要件であり、そのまま残る

## テスト設計

判定の実装を外部に移すため、このリポジトリに自動テストは置かない。次の手順で確かめる。

- `adr-guard links` が終了コード 0 で終わること
- 変更をすべてステージした状態で、`.githooks/pre-commit` が終了コード 0 で終わること（ステータスの行の変更と、リンクの付与が許可されること）
- コミット済みの ADR の本文を一時的に書き換えてステージすると、`.githooks/pre-commit` が終了コード 1 で拒否すること。確かめた後に戻す
- `adr-guard` が PATH にないとき、pre-commit hook がコミットを拒否すること
- pull request の CI で、`adr` ジョブが次のとおり動いて成功すること
  - ログで `adr-guard-v0.5.0-x86_64-unknown-linux-gnu.tar.gz` のチェックサムが検証されていること
  - `check-range --allow-status --allow-links` と `links` が実行されていること
  - pull request には、コミット済みの ADR のステータスの行の変更とリンクの付与が含まれるため、二つのオプションが効いていることも確かめられる
- ほかのジョブ（Frontend、E2E、Tauri）が、`scripts/` と `test/` を消しても通ること

## トレードオフ

- 開発環境に、Deno の代わりに adr-guard が要る。adr-guard がないと、pre-commit hook はコミットを拒否する
- Claude Code の hook が全体の設定に依るため、全体の設定を持たない環境では、Claude Code による ADR の書き換えを止められない。pre-commit hook と CI で検査する
- Bash による書き換えを実行の直後に差し戻せなくなる。コミットのときまで気づかない
- CI で ADR の変更を検査するため、範囲内で一度でも ADR を書き換えたコミットがあると、後のコミットで戻しても CI が失敗する。履歴を書き換えて（rebase などで）直す必要がある
- リンクの付与のため、既存の ADR と仕様書の差分が大きくなる。一つのコミットに分けて、レビューしやすくする
- 判定の実装がこのリポジトリの外にあるため、規則の変更は adr-guard の版の更新として受け取る。版は action のタグで固定する
