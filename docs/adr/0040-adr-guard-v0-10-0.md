# ADR 0040: adr-guard を v0.10.0 に上げる

ステータス: 採択

## 文脈

[ADR 0039](0039-adr-guard-v0-8-3.md) で、adr-guard を v0.8.3 に上げた。
pre-commit hook は `adr-guard pre-commit --allow-status --allow-links`、CI は `tett23/adr-guard@v0.8.3` の composite action（`allow-status: true`、`allow-links: true`）を使っている。

adr-guard の v0.9.0 から v0.10.0 までで、次のことが変わった。手元の adr-guard は v0.10.0 である。

- v0.9.0 で、同じディレクトリで番号が重複した ADR を報告する `adr-guard check-numbers` と、重複を直す `adr-guard renumber [--dry-run]` が加わった。composite action は既定で `check-numbers` も実行し（入力 `check-numbers`）、重複があると CI が失敗する。番号を付け直すコミットを許す `--allow-renumber`（action の入力 `allow-renumber`、既定 false）も加わった（[adr-guard の ADR 0018](https://github.com/tett23/adr-guard/blob/main/docs/adr/0018-repair-duplicate-adr-numbers.md)、[v0.9.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.9.0)）
- v0.9.1 で、`renumber` の誤報が直った（[v0.9.1 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.9.1)）
- v0.10.0 で、修飾子付きの ADR の参照を扱うようになった。`ADR NNNN` の直前の、空白1つで区切った語が修飾子になる。`/` を含む語はディレクトリ、`-` を含む語はモジュール（`docs/<モジュール>/adr/`）、それ以外の語はそのモジュールがあるときだけ修飾子になる。修飾子はリンクのテキストに含める。`links` と pre-commit hook は、ディレクトリや ADR が存在しない修飾子付きの参照、リンク先が修飾子の ADR と異なるリンク、リンクの外に出た修飾子を報告する（[adr-guard の ADR 0019](https://github.com/tett23/adr-guard/blob/main/docs/adr/0019-module-qualified-adr-references.md)）
- v0.10.0 で、シンボリックリンクの `.md` を検査しなくなった（[adr-guard の ADR 0020](https://github.com/tett23/adr-guard/blob/main/docs/adr/0020-skip-symlinked-documents.md)、[v0.10.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.10.0)）

このリポジトリの ADR は `docs/adr/` だけにあり、モジュールの ADR はない。ほかのリポジトリの ADR への参照は、`epub-builder の [ADR 0019](https://github.com/...)` のように、`の` を挟んで外部リンクにしている。手元の adr-guard 0.10.0 で、`adr-guard links`、`adr-guard check-status`、`adr-guard check-numbers` はいずれも終了コード 0 で終わり、`adr-guard links --fix --include-committed` はどのファイルも書きかえない。

## 実装すること

- CI（`.github/workflows/ci.yml`）の `adr` ジョブの action を `uses: tett23/adr-guard@v0.10.0` にする。`allow-status: true` と `allow-links: true` はそのまま残し、`check-numbers` と `allow-renumber` は既定のままにする
- `.githooks/pre-commit` は `exec adr-guard pre-commit --allow-status --allow-links` のまま変えない
- README の「開発」の adr-guard のインストールを `cargo install --git https://github.com/tett23/adr-guard --tag v0.10.0` にし、ADR の番号の重複を `adr-guard check-numbers` で調べられること、CI が番号の重複も検査することを書く
- `docs/specifications.md` の「開発」の CI の action の版を v0.10.0 にし、`check-numbers` で ADR の番号の重複も検査することを書く
- [ADR 0039](0039-adr-guard-v0-8-3.md) のステータスを `採択→[ADR 0040](0040-adr-guard-v0-10-0.md)` にし、変えた内容（adr-guard の版）を `docs/specifications.md` の「ADR の状態」の表に書く

## 実装しないこと

- `--allow-renumber` を pre-commit hook や CI で有効にすること。番号の付け直しは、必要になったときに改めて決める
- 修飾子付きの参照の規則を README や仕様書に書くこと。このリポジトリにはモジュールの ADR がなく、ほかのリポジトリの ADR は外部リンクで参照している
- Claude Code の hook（全体の設定にある `adr-guard claude-hook --allow-status`）の変更。利用者の全体の設定であり、このリポジトリでは扱わない

## テスト設計

設定と文書の変更であり、このリポジトリに adr-guard の設定を検査する自動テストはない。次の手順で確かめる。

- 手元の adr-guard 0.10.0 で、`adr-guard links`、`adr-guard check-status`、`adr-guard check-numbers` が終了コード 0 で終わること
- 手元の adr-guard 0.10.0 で、`adr-guard links --fix --include-committed` がどのファイルも書きかえないこと
- 変更をすべてステージした状態で、`.githooks/pre-commit` が終了コード 0 で終わること（[ADR 0039](0039-adr-guard-v0-8-3.md) のステータスの行の変更が許可されること）
- コミットした後、`adr-guard check-range origin/main HEAD --allow-status --allow-links` が終了コード 0 で終わること
- `.github/workflows/ci.yml` が YAML として読めること
- pull request の CI で、`adr` ジョブが次のとおり動いて成功すること
  - ログで `adr-guard-v0.10.0-x86_64-unknown-linux-gnu.tar.gz` のチェックサムが検証されていること
  - `check-range --allow-status --allow-links`、`links`、`check-status`、`check-numbers` が実行されていること

## トレードオフ

- 手元の adr-guard を v0.10.0 に上げる必要がある。古い版の pre-commit hook は、修飾子付きの参照の誤りを見逃し、CI で初めて失敗する
- 別のブランチで同じ番号の ADR を作ると、マージした後の CI が失敗する。`adr-guard renumber` で直せるが、番号を付け直すコミットには `--allow-renumber` が要る
- ほかのリポジトリの ADR を `<リポジトリ名> ADR NNNN` の形で書くと、修飾子として扱われて存在しないモジュールの参照として報告される。`の` を挟むか、リンクのテキストごと外部リンクにする

## 参考文献

- [adr-guard の ADR 0018: 重複したADRの番号の修復](https://github.com/tett23/adr-guard/blob/main/docs/adr/0018-repair-duplicate-adr-numbers.md)
- [adr-guard の ADR 0019: モジュールで修飾したADRの参照](https://github.com/tett23/adr-guard/blob/main/docs/adr/0019-module-qualified-adr-references.md)
- [adr-guard の ADR 0020: シンボリックリンクの文書を飛ばす](https://github.com/tett23/adr-guard/blob/main/docs/adr/0020-skip-symlinked-documents.md)
- [adr-guard v0.9.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.9.0)
- [adr-guard v0.9.1 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.9.1)
- [adr-guard v0.10.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.10.0)
