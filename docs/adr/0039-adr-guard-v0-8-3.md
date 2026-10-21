# ADR 0039: adr-guard を v0.8.3 に上げる

ステータス: 採択

## 文脈

[ADR 0038](0038-adr-guard-v0-8-2.md) で、adr-guard を v0.8.2 に上げた。
pre-commit hook は `adr-guard pre-commit --allow-status --allow-links`、CI は `tett23/adr-guard@v0.8.2` の composite action（`allow-status: true`、`allow-links: true`）を使っている。

adr-guard の v0.8.3 では、ADR のステータスに `採択（未実装）` が加わった（[adr-guard の ADR 0016](https://github.com/tett23/adr-guard/blob/main/docs/adr/0016-accepted-not-implemented-status.md)、[リリース](https://github.com/tett23/adr-guard/releases/tag/v0.8.3)）。手元の adr-guard は v0.8.3 である。

- `採択（未実装）` は、採択されたがまだ実装されていない ADR を表す。括弧は全角でも半角でもよい。英語では `Accepted (not implemented)` と書く
- 新しいサブコマンド `adr-guard unimplemented [--status]` で、ステータスが `採択（未実装）` の ADR を一覧できる

利用者の全体の設定（CLAUDE.md）でも、ADR のステータスの候補に `採択（未実装）` が加わった。
このリポジトリの `docs/specifications.md` の「ADR の状態」は、ステータスの候補を `採択`、`採択→<変更した ADR>`、`提案`、`却下` に限っている。

## 実装すること

- CI（`.github/workflows/ci.yml`）の `adr` ジョブの action を `uses: tett23/adr-guard@v0.8.3` にする。`allow-status: true` と `allow-links: true` はそのまま残す
- `.githooks/pre-commit` は `exec adr-guard pre-commit --allow-status --allow-links` のまま変えない
- README の「開発」の adr-guard のインストールを `cargo install --git https://github.com/tett23/adr-guard --tag v0.8.3` にし、まだ実装していない ADR を `adr-guard unimplemented` で一覧できることを書く
- `docs/specifications.md` の「開発」の CI の action の版を v0.8.3 にする
- `docs/specifications.md` の「ADR の状態」のステータスの候補に `採択（未実装）` を加える。採択したが実装していない ADR に使い、実装したら `採択` にする
- [ADR 0038](0038-adr-guard-v0-8-2.md) のステータスを `採択→[ADR 0039](0039-adr-guard-v0-8-3.md)` にし、変えた内容（adr-guard の版）を `docs/specifications.md` の「ADR の状態」の表に書く

## 実装しないこと

- 既存の ADR のステータスを `採択（未実装）` に書き換えること。現在の採択の ADR は実装済みである
- `adr-guard unimplemented` を CI で実行すること。未実装の ADR があることは違反ではない
- Claude Code の hook（全体の設定にある `adr-guard claude-hook --allow-status`）の変更。利用者の全体の設定であり、このリポジトリでは扱わない

## テスト設計

設定と文書の変更であり、このリポジトリに ADR のステータスの書式を検査する自動テストはない。次の手順で確かめる。

- 手元の adr-guard 0.8.3 で、`adr-guard check-status` と `adr-guard links` が終了コード 0 で終わること
- 手元の adr-guard 0.8.3 で、`adr-guard unimplemented` が終了コード 0 で終わること
- 変更をすべてステージした状態で、`.githooks/pre-commit` が終了コード 0 で終わること（[ADR 0038](0038-adr-guard-v0-8-2.md) のステータスの行の変更が許可されること）
- `.github/workflows/ci.yml` が YAML として読めること
- pull request の CI で、`adr` ジョブが次のとおり動いて成功すること
  - ログで `adr-guard-v0.8.3-x86_64-unknown-linux-gnu.tar.gz` のチェックサムが検証されていること
  - `check-range --allow-status --allow-links`、`links`、`check-status` が実行されていること

## トレードオフ

- 手元の adr-guard を v0.8.3 に上げる必要がある。古い版でも `採択（未実装）` は違反にならないが、`adr-guard unimplemented` は使えない
- ステータスの候補が増えるため、実装した後に `採択（未実装）` を `採択` に戻し忘れる恐れがある。`adr-guard unimplemented` の一覧で確かめる

## 参考文献

- [adr-guard の ADR 0016: ステータス `採択（未実装）` を加え、未実装のADRを一覧する `unimplemented` サブコマンド](https://github.com/tett23/adr-guard/blob/main/docs/adr/0016-accepted-not-implemented-status.md)
- [adr-guard v0.8.3 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.8.3)
