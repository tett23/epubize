# ADR 0036: adr-guard を v0.8.0 に上げる

ステータス: 採択→[ADR 0037](0037-adr-guard-v0-8-1.md)

## 文脈

[ADR 0035](0035-use-external-adr-guard.md) で、コミット済みの ADR の検査を [adr-guard](https://github.com/tett23/adr-guard) の v0.5.0 に任せた。
pre-commit hook は `adr-guard pre-commit --allow-status --allow-links`、CI は `tett23/adr-guard@v0.5.0` の composite action（`allow-status: true`、`allow-links: true`）を使っている。

adr-guard は v0.8.0 まで進み、規則が次のとおり変わった。手元の adr-guard は v0.8.0 である。

- v0.6.0：どの深さの `docs/adr/` と `docs/<モジュール>/adr/` も保護する（[adr-guard の ADR 0011](https://github.com/tett23/adr-guard/blob/main/docs/adr/0011-nested-adr-directories.md)）。このリポジトリの ADR は `docs/adr/` だけなので、影響はない
- v0.7.0：ADR は、行頭が `ステータス:` か `Status:` のステータスの行をちょうど1つ持たねばならない。無い ADR と2つ以上ある ADR は違反になる。サブコマンド `adr-guard check-status` が加わり、action は既定で `check-status` も実行する。`--allow-status` で、ステータスの行が無かった ADR への1行の追加も許可される（[adr-guard の ADR 0012](https://github.com/tett23/adr-guard/blob/main/docs/adr/0012-status-line-and-status-adt.md)）
- v0.8.0：`docs/audit/` と `docs/benchmarks/`（どの深さの `docs/` でも）のコミット済みのファイルも、変更・削除・移動できない。`--allow-status` と `--allow-links` は、これらの記録には効かない（[adr-guard の ADR 0013](https://github.com/tett23/adr-guard/blob/main/docs/adr/0013-protect-audit-and-benchmark-records.md)）

このリポジトリの ADR（0001 から 0035）は、どれも `ステータス:` の行をちょうど1つ持ち、v0.8.0 の `adr-guard check-status` は終了コード 0 で終わる。
`docs/audit/` と `docs/benchmarks/` は、まだない。

## 実装すること

- CI（`.github/workflows/ci.yml`）の `adr` ジョブの action を `uses: tett23/adr-guard@v0.8.0` にする。`allow-status: true` と `allow-links: true` はそのまま残す
  - action は `check-range` と `links` に加えて、既定で `check-status` も実行する。`check-status: false` は付けない
- `.githooks/pre-commit` は `exec adr-guard pre-commit --allow-status --allow-links` のまま変えない
- README の「開発」の adr-guard のインストールを `cargo install --git https://github.com/tett23/adr-guard --tag v0.8.0` にし、ADR がステータスの行をちょうど1つ持つこと、`adr-guard check-status` での確かめ方、`docs/audit/` と `docs/benchmarks/` の記録も変更できないことを書く
- `docs/specifications.md` の「開発」と「ADR の状態」を、上の内容に合わせる
- [ADR 0035](0035-use-external-adr-guard.md) のステータスを `採択→[ADR 0036](0036-adr-guard-v0-8-0.md)` にし、変えた内容（adr-guard の版）を `docs/specifications.md` の「ADR の状態」の表に書く

## 実装しないこと

- `docs/audit/` と `docs/benchmarks/` を作ること。記録を置くときに、別に決める
- ステータスの行の移行。違反のある ADR がないため
- Claude Code の hook（全体の設定にある `adr-guard claude-hook --allow-status`）の変更。利用者の全体の設定であり、このリポジトリでは扱わない

## テスト設計

設定と文書の変更であり、このリポジトリに自動テストは置かない。次の手順で確かめる。

- 手元の adr-guard 0.8.0 で、`adr-guard check-status` と `adr-guard links` が終了コード 0 で終わること
- 変更をすべてステージした状態で、`.githooks/pre-commit` が終了コード 0 で終わること（[ADR 0035](0035-use-external-adr-guard.md) のステータスの行の変更が許可されること）
- コミット済みの ADR の本文を一時的に書き換えてステージすると、`.githooks/pre-commit` が終了コード 1 で拒否すること。確かめた後に戻す
- `.github/workflows/ci.yml` が YAML として読めること
- pull request の CI で、`adr` ジョブが次のとおり動いて成功すること
  - ログで `adr-guard-v0.8.0-x86_64-unknown-linux-gnu.tar.gz` のチェックサムが検証されていること
  - `check-range --allow-status --allow-links`、`links`、`check-status` が実行されていること

## トレードオフ

- 手元の adr-guard を v0.8.0 に上げる必要がある。古い版のままでも pre-commit hook は動くが、ステータスの行の数と記録の保護は CI でしか検査されない
- ステータスの行が無い ADR や2つある ADR をコミットできなくなる。ADR の書式の誤りを早く見つけられる代わりに、ADR の雛形を守る必要がある
- 将来 `docs/audit/` や `docs/benchmarks/` に記録を置くと、コミットした後は誤記も直せない。直す場合は新しい記録を作る
