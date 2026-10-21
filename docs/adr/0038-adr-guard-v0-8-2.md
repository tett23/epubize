# ADR 0038: adr-guard を v0.8.2 に上げる

ステータス: 採択→[ADR 0039](0039-adr-guard-v0-8-3.md)

## 文脈

[ADR 0037](0037-adr-guard-v0-8-1.md) で、adr-guard を v0.8.1 に上げた。
pre-commit hook は `adr-guard pre-commit --allow-status --allow-links`、CI は `tett23/adr-guard@v0.8.1` の composite action（`allow-status: true`、`allow-links: true`）を使っている。

adr-guard の v0.8.2 では、`--allow-links` が許す変更が広がった（[adr-guard の ADR 0015](https://github.com/tett23/adr-guard/blob/main/docs/adr/0015-allow-link-changes.md)、[リリース](https://github.com/tett23/adr-guard/releases/tag/v0.8.2)）。手元の adr-guard は v0.8.2 である。

- これまでは、コミット済みの ADR で、ADR の参照にリンクを付ける変更だけを許していた
- v0.8.2 では、コミット済みの ADR の「リンクだけの変更」を広く許す。インラインリンクと参照形式のリンクの追加・削除・リンク先の変更、画像のリンク先の変更、リンクの参照定義の行（`[1]: url`）の変更である
- リンクのテキストの変更、画像の追加・削除、自動リンク（`<url>`）や本文の URL の変更は、引き続き拒否する
- 監査や計測の記録（`docs/audit/` など）は、引き続き `--allow-links` が効かず、完全に保護する

## 実装すること

- CI（`.github/workflows/ci.yml`）の `adr` ジョブの action を `uses: tett23/adr-guard@v0.8.2` にする。`allow-status: true` と `allow-links: true` はそのまま残す
- `.githooks/pre-commit` は `exec adr-guard pre-commit --allow-status --allow-links` のまま変えない
- README の「開発」の adr-guard のインストールを `cargo install --git https://github.com/tett23/adr-guard --tag v0.8.2` にし、コミット済みの ADR で変えてよいものを「ステータスの行と、リンクだけの変更」にする
- `docs/specifications.md` の「開発」と「ADR の状態」を、上の内容に合わせる
- [ADR 0037](0037-adr-guard-v0-8-1.md) のステータスを `採択→[ADR 0038](0038-adr-guard-v0-8-2.md)` にし、変えた内容（adr-guard の版）を `docs/specifications.md` の「ADR の状態」の表に書く

## 実装しないこと

- `--allow-links` を外して、リンクの変更を以前より狭く制限すること。リンク先の変更は ADR の決定を変えないので、許してよい
- コミット済みの ADR のリンクを、この機会にまとめて書き換えること。必要になったときに直す
- Claude Code の hook（全体の設定にある `adr-guard claude-hook --allow-status`）の変更。利用者の全体の設定であり、このリポジトリでは扱わない

## テスト設計

設定と文書の変更であり、このリポジトリに自動テストは置かない。次の手順で確かめる。

- 手元の adr-guard 0.8.2 で、`adr-guard check-status` と `adr-guard links` が終了コード 0 で終わること
- 変更をすべてステージした状態で、`.githooks/pre-commit` が終了コード 0 で終わること（[ADR 0037](0037-adr-guard-v0-8-1.md) のステータスの行の変更が許可されること）
- `.github/workflows/ci.yml` が YAML として読めること
- pull request の CI で、`adr` ジョブが次のとおり動いて成功すること
  - ログで `adr-guard-v0.8.2-x86_64-unknown-linux-gnu.tar.gz` のチェックサムが検証されていること
  - `check-range --allow-status --allow-links`、`links`、`check-status` が実行されていること

## トレードオフ

- 手元の adr-guard を v0.8.2 に上げる必要がある。古い版のままだと、ADR の参照以外のリンクの変更を pre-commit hook が拒否する（CI では許される）
- コミット済みの ADR のリンク先を変えられるようになるため、リンク先の差し替えで ADR の根拠の文献が変わっても検査では止まらない。リンク先の変更はレビューで確かめる

## 参考文献

- [adr-guard の ADR 0015: `--allow-links` で、ADRの参照以外のリンクを付ける・外す・リンク先を変える変更も許可する](https://github.com/tett23/adr-guard/blob/main/docs/adr/0015-allow-link-changes.md)
- [adr-guard v0.8.2 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.8.2)
