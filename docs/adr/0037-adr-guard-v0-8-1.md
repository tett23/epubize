# ADR 0037: adr-guard を v0.8.1 に上げる

ステータス: 採択

## 文脈

[ADR 0036](0036-adr-guard-v0-8-0.md) で、adr-guard を v0.8.0 に上げた。
pre-commit hook は `adr-guard pre-commit --allow-status --allow-links`、CI は `tett23/adr-guard@v0.8.0` の composite action（`allow-status: true`、`allow-links: true`）を使っている。

adr-guard の v0.8.1 では、保護するディレクトリの名前の揺れも認めるようになった（[adr-guard の ADR 0014](https://github.com/tett23/adr-guard/blob/main/docs/adr/0014-directory-name-variants.md)、[リリース](https://github.com/tett23/adr-guard/releases/tag/v0.8.1)）。手元の adr-guard は v0.8.1 である。

- ADR のディレクトリとして、`adr` のほか `adrs`、`decisions`、`decision` も保護する
- 記録のディレクトリとして、`audit` と `benchmarks` のほか `audits` と `benchmark` も保護する（どの深さの `docs/` でも）

このリポジトリの ADR は `docs/adr/` だけにある。新しく保護される名前のディレクトリは、追跡しているファイルにも、追跡していないディレクトリにもない。
そのため、保護の範囲が広がっても、既存のファイルの扱いは変わらない。

## 実装すること

- CI（`.github/workflows/ci.yml`）の `adr` ジョブの action を `uses: tett23/adr-guard@v0.8.1` にする。`allow-status: true` と `allow-links: true` はそのまま残す
- `.githooks/pre-commit` は `exec adr-guard pre-commit --allow-status --allow-links` のまま変えない
- README の「開発」の adr-guard のインストールを `cargo install --git https://github.com/tett23/adr-guard --tag v0.8.1` にし、変更できない記録のディレクトリに `docs/audits/` と `docs/benchmark/` を加える
- `docs/specifications.md` の「開発」と「ADR の状態」を、上の内容に合わせる
- [ADR 0036](0036-adr-guard-v0-8-0.md) のステータスを `採択→[ADR 0037](0037-adr-guard-v0-8-1.md)` にし、変えた内容（adr-guard の版）を `docs/specifications.md` の「ADR の状態」の表に書く

## 実装しないこと

- `docs/audits/` や `docs/benchmark/` など、記録のディレクトリを作ること。記録を置くときに、別に決める
- ADR の置き場所を `docs/adrs/` などに移すこと。`docs/adr/` のまま使う
- Claude Code の hook（全体の設定にある `adr-guard claude-hook --allow-status`）の変更。利用者の全体の設定であり、このリポジトリでは扱わない

## テスト設計

設定と文書の変更であり、このリポジトリに自動テストは置かない。次の手順で確かめる。

- 手元の adr-guard 0.8.1 で、`adr-guard check-status` と `adr-guard links` が終了コード 0 で終わること
- 変更をすべてステージした状態で、`.githooks/pre-commit` が終了コード 0 で終わること（[ADR 0036](0036-adr-guard-v0-8-0.md) のステータスの行の変更が許可されること）
- `.github/workflows/ci.yml` が YAML として読めること
- pull request の CI で、`adr` ジョブが次のとおり動いて成功すること
  - ログで `adr-guard-v0.8.1-x86_64-unknown-linux-gnu.tar.gz` のチェックサムが検証されていること
  - `check-range --allow-status --allow-links`、`links`、`check-status` が実行されていること

## トレードオフ

- 手元の adr-guard を v0.8.1 に上げる必要がある。古い版のままでも pre-commit hook は動くが、新しく保護される名前のディレクトリは CI でしか検査されない
- 将来 `docs/` 以下に `adrs`、`decisions`、`decision`、`audits`、`benchmark` という名前のディレクトリを、ADR や記録のつもりでなく作ると、コミットした後は変更できなくなる。その名前は避ける
