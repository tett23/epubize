# ADR 0041: adr-guard を v0.13.1 に上げ、ADR のステータスの行を新形式に移す

Status: accepted

## 文脈

[ADR 0040](0040-adr-guard-v0-10-0.md) で、adr-guard を v0.10.0 に上げた。
pre-commit hook は `adr-guard pre-commit --allow-status --allow-links`、CI は `tett23/adr-guard@v0.10.0` の composite action（`allow-status: true`、`allow-links: true`）を使っている。

adr-guard の v0.11.0 から v0.13.1 までで、次のことが変わった。手元の adr-guard は v0.13.1 である。

- v0.11.0 で、Codex の PreToolUse hook として動く `adr-guard codex-hook` が加わった（[adr-guard の ADR 0021](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0021-codex-hook.md)、[v0.11.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.11.0)）
- v0.12.0 で、ステータスの行の新形式と、状態遷移の検査が加わった（[adr-guard の ADR 0022](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0022-status-transitions.md)、[v0.12.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.12.0)）
  - 新形式は `Status: <トークン>` である。トークンは `proposed`、`accepted`、`accepted-pending`、`rejected`、`discarded`、`deprecated` のいずれか、または `superseded [<変更の JSON 配列>]` である。変更は `{"part":<部分名か null>,"by":[<リポジトリのルートからのパス>]}` と書く
  - 追加した ADR と、ステータスの行を書き換えた ADR には、新形式が要る。新しい ADR は、最初のコミットで `Status: proposed` でなければならない
  - pre-commit hook と `check-range` は、HEAD からの状態遷移を検査する。遷移の違反は `--allow-status`、環境変数 `ADR_GUARD_ALLOW`、コミットのトレーラーでも免除されない
  - 旧形式（`ステータス: 採択` など）の行は、書き換えない限りそのまま読める
  - 旧形式を新形式に書き換える移行スクリプト `scripts/migrate-adr-status` が adr-guard のソースにある。リリースのアーカイブには入らない
  - 修正先を辿る `adr-guard trace` が加わった（[adr-guard の ADR 0023](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0023-trace-amendments.md)）
- v0.13.0 で、移行スクリプトに `--dry-run` が加わり、移行できない ADR を全件数えるようになった（[adr-guard の ADR 0024](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0024-migration-dry-run-and-error-count.md)）。状態 `superseded-pending`（実装を待つ ADR が修正を受けた状態）、変更の `kind` と `note`、`https://` の修正先、`trace --list` の6列が加わった（[adr-guard の ADR 0025](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0025-extend-canonical-status-amendments.md)）。旧形式の参照の読み方が広がった（[adr-guard の ADR 0026](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0026-read-more-legacy-references.md)、[adr-guard の ADR 0027](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0027-read-legacy-replaced-status.md)、[v0.13.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.13.0)）
- v0.13.1 で、移行スクリプトが旧形式の行から部分名を作るときに、修飾子（`ADR NNNN` の直前の語）、先頭の「。」、末尾の「→」を残さなくなった。部分名を規則で決められない行は、「部分名を決められません」と報告し、書き換えない（[adr-guard の ADR 0028](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0028-fix-legacy-part-names.md)、[v0.13.1 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.13.1)）

このリポジトリの ADR は、すべて旧形式のステータスの行を持つ。
`docs/specifications.md` の「16. ADR の状態」は、ステータスの値を `採択`、`採択（未実装）`、`採択→<変更した ADR>`、`提案`、`却下` と決め、後の ADR で変えた ADR を、旧形式のステータスとともに表にしている。

## 調査した事項

次は、この ADR の worktree（origin/main から作ったブランチ `adr-guard-v0.13.0`）で実行して確かめた。

- `adr-guard -V` は `adr-guard 0.13.1` を出力した
- `.github/workflows/ci.yml` の `adr` ジョブは `uses: tett23/adr-guard@v0.10.0` で、入力は `base`、`head`、`allow-status: true`、`allow-links: true` である。その前の行のコメントに [ADR 0035](0035-use-external-adr-guard.md) から [ADR 0040](0040-adr-guard-v0-10-0.md) までの番号がある
- `.githooks/pre-commit` は `exec adr-guard pre-commit --allow-status --allow-links` を実行する。`.claude/` には `launch.json` だけがあり、hook の設定は無い
- adr-guard の v0.13.1 のタグはコミット `ab55769152ed0f8a3465ee3435a2df43ab9d067f` を指す。v0.13.1 の `action.yml` の入力は `base`、`head`、`version`、`allow-status`、`allow-links`、`links`、`check-status`、`check-numbers`、`allow-renumber` である
- この ADR を足す前に、`adr-guard links`、`adr-guard check-status`、`adr-guard check-numbers` は、いずれも終了コード 0 で終わった
- worktree のルートで、adr-guard の v0.13.1 のソースの `scripts/migrate-adr-status`（ビルドしたもの）を `--dry-run` で実行した。この ADR を足す前は、終了コード 0 で、標準エラーに `移行できる: 40件、移行済み: 0件、移行できない: 0件` を出力した。40 件の内訳は `Status: accepted` が 16 件、`Status: superseded [...]` が 24 件である
- v0.13.1 の移行スクリプトの `--dry-run` が出す新形式のステータスの行は、v0.13.0 の移行スクリプトの出力とすべて同じだった（`diff` で差が無かった）
- 移行スクリプトは、[ADR 0039](0039-adr-guard-v0-8-3.md) の `採択→[ADR 0040](0040-adr-guard-v0-10-0.md)` を `Status: superseded [{"by":["docs/adr/0040-adr-guard-v0-10-0.md"],"part":null}]` に書き換える。部分名の無い旧形式の行は、`part` が `null` になる
- worktree を `$TMPDIR` に clone し、実装の手順を試した。この ADR を `Status: proposed` で足したインデックスで、`adr-guard pre-commit --allow-status --allow-links` は終了コード 0 で終わった。それをコミットし、ステータスを `Status: accepted-pending` にしたインデックスでも、終了コード 0 で終わった
- 続けて、移行スクリプトの `--dry-run` は `移行できる: 40件、移行済み: 1件、移行できない: 0件`（移行済みの 1 件はこの ADR）を出力した。`--write` で書き換え、[ADR 0040](0040-adr-guard-v0-10-0.md) のステータスの行を「実装すること」の行にしたインデックスで、pre-commit は終了コード 0 で終わった
- そのコミットの後、`--dry-run` は `移行できる: 0件、移行済み: 41件、移行できない: 0件` を出力した。`adr-guard check-status`、`adr-guard links`、`adr-guard check-numbers`、`adr-guard check-range origin/main HEAD --allow-status --allow-links` は、いずれも終了コード 0 で終わった。`^ステータス:` で始まる行は残らなかった。`adr-guard trace --list docs/adr/0035-use-external-adr-guard.md` は、[ADR 0035](0035-use-external-adr-guard.md) から [ADR 0040](0040-adr-guard-v0-10-0.md) を経てこの ADR に至る辺を、部分名なしで出した
- 旧形式の ADR を足した main のコミット cb713e0（[ADR 0040](0040-adr-guard-v0-10-0.md) を足したもの）で `adr-guard check-range cb713e0^ cb713e0 --allow-status --allow-links` を実行すると、「新形式のステータス行は `Status: ` で始めてください」 を出して終了コード 1 で終わった。[ADR 0039](0039-adr-guard-v0-8-3.md) のステータスを旧形式のまま書き換えたコミット f97d98b でも、同じ診断を出した
- adr-guard の版とステータスの行の書き方を書いた文書は、README の「開発」（`--tag v0.10.0`、行頭の `ステータス:`、`採択（未実装）`）と、`docs/specifications.md` の「14. 開発」（`tett23/adr-guard@v0.10.0`、`ステータス:` の行）と「16. ADR の状態」である

## 実装すること

- CI（`.github/workflows/ci.yml`）の `adr` ジョブの action を `uses: tett23/adr-guard@v0.13.1` にする。`allow-status: true` と `allow-links: true` はそのまま残し、`check-numbers` と `allow-renumber` は既定のままにする。コメントの ADR の番号にこの ADR を加える
- `.githooks/pre-commit` は `exec adr-guard pre-commit --allow-status --allow-links` のまま変えない
- worktree のルートで、adr-guard の v0.13.1 のソースの `scripts/migrate-adr-status` を `--dry-run` で実行し、移行できない ADR が 0 件であることを確かめてから、`--write` ですべての ADR のステータスの行を新形式に書き換える
- [ADR 0040](0040-adr-guard-v0-10-0.md) のステータスを `Status: superseded [{"part":null,"by":["docs/adr/0041-adr-guard-v0-13-1.md"]}]` にし、変えた内容（adr-guard の版）を `docs/specifications.md` の「ADR の状態」の表に書く
- README の「開発」の adr-guard のインストールを `cargo install --git https://github.com/tett23/adr-guard --tag v0.13.1` にし、ステータスの行が `Status:` で始まること、まだ実装していない ADR のステータスが `accepted-pending` であることを書く。参照する ADR にこの ADR を加える
- `docs/specifications.md` の「14. 開発」の CI の action の版を v0.13.1 にし、ステータスの行を `Status:` の行と書き、pre-commit hook と CI が状態遷移を検査することを書く
- `docs/specifications.md` の「16. ADR の状態」を、次の内容にする
  - ステータスは、各 ADR の 3 行目の `Status:` の行（ちょうど 1 つ）に書く。値は `proposed`（提案）、`accepted-pending`（採択したがまだ実装していない）、`accepted`（採択して実装した）、`superseded [...]`（後の ADR で決定の全部または一部を変えた）、`rejected`（却下）のいずれかとする
  - 新しい ADR は、レビューを受けたあと `proposed` で一度コミットし、続くコミットで `accepted-pending` か `accepted` にする。`accepted-pending` の ADR は、実装したら `accepted` にする。`accepted-pending` の ADR は `adr-guard unimplemented` で一覧できる
  - 後の ADR で決定の全部または一部を変えた ADR は `superseded [{"part":null,"by":["docs/adr/NNNN-….md"]}]` とし、変えた内容は表に書く
  - 表の「ステータス」の列は、旧形式の文字列をやめて「変えた ADR」の列にし、変えた ADR へのリンクを並べる
- `adr-guard links --fix` を実行し、`adr-guard links`、`adr-guard check-status`、`adr-guard check-numbers` が終了コード 0 で終わるようにする

## 実装しないこと

- Claude Code の hook（全体の設定にある `adr-guard claude-hook --allow-status`）の変更。利用者の全体の設定であり、このリポジトリでは扱わない
- Codex の hook（v0.11.0 の `adr-guard codex-hook`）の導入
- 変更に部分名（`part`）を新たに書くこと。旧形式で部分名を書いていないこのリポジトリの慣習に倣い、変えた内容は「ADR の状態」の表に書く
- 変更の `kind`（`supplement`、`note`）と `note`、`https://` の修正先、`superseded-pending`、`deprecated` を使うこと
- 移行スクリプトをこのリポジトリに置くこと。移行は一度だけ行い、adr-guard の v0.13.1 のソースから実行する

## 不変条件

なし。
この ADR は、CI の版、ADR のステータスの行の書き方、その手順だけを決め、epubize の振る舞いを変えない。

## テスト設計

設定と文書の変更であり、このリポジトリに adr-guard の設定を検査する自動テストはない。
次の手順で確かめる。

- 移行スクリプトの `--write` の後に `--dry-run` を実行すると、`移行できる: 0件` で、移行できない ADR が 0 件であること
- `rg -n '^ステータス:' docs/adr` が何も出力しないこと
- 手元の adr-guard 0.13.1 で、`adr-guard links`、`adr-guard check-status`、`adr-guard check-numbers` が終了コード 0 で終わること
- 変更をコミットごとにステージした状態で、`.githooks/pre-commit` が終了コード 0 で終わること（この ADR の `proposed` での追加、採択、移行と [ADR 0040](0040-adr-guard-v0-10-0.md) のステータスの行の変更が許可されること）
- コミットした後、`adr-guard check-range origin/main HEAD --allow-status --allow-links` が終了コード 0 で終わること
- `adr-guard trace --list docs/adr/0035-use-external-adr-guard.md` が、[ADR 0040](0040-adr-guard-v0-10-0.md) からこの ADR への辺を出すこと
- `.github/workflows/ci.yml` が YAML として読めること
- pull request の CI で、`adr` ジョブが次のとおり動いて成功すること
  - ログで `adr-guard-v0.13.1-x86_64-unknown-linux-gnu.tar.gz` のチェックサムが検証されていること
  - `check-range --allow-status --allow-links`、`links`、`check-status`、`check-numbers` が実行されていること

## トレードオフ

- 手元と CI の版がそろい、新しい ADR を新形式で書ける。その代わり、ADR を足すたびに `proposed` のコミットと採択のコミットの 2 つが要る
- 状態遷移の違反は `--allow-status` でも免除されないので、誤ったステータスの書き換えを手元と CI で止められる。その代わり、誤って入れたステータスを戻すには、許された遷移を辿る必要がある
- CI を上げた後は、旧形式のステータスの行で ADR を足すコミットと、旧形式の行を旧形式のまま書き換えるコミットが、CI で失敗する。移行より前に分けたブランチにそのようなコミットがあれば、マージの前にステータスの行を新形式に直す
- 40 件の ADR のステータスの行を一度に書き換えるので、差分は大きい。書き換えはステータスの行だけで、`--allow-status` の範囲に収まる
- 手元の adr-guard を v0.13.1 に上げる必要がある。古い版の pre-commit hook は、状態遷移の誤りを見逃し、CI で初めて失敗する

## 参考文献

- [adr-guard の ADR 0021: Codex の PreToolUse hook で、コミット済みのADRと記録の編集を拒否する](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0021-codex-hook.md)
- [adr-guard の ADR 0022: 状態と遷移を別々の代数的データ型で表す](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0022-status-transitions.md)
- [adr-guard の ADR 0023: trace で元の決定から修正ADRをたどる](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0023-trace-amendments.md)
- [adr-guard の ADR 0024: 移行スクリプトに --dry-run を設け、失敗を最後まで数える](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0024-migration-dry-run-and-error-count.md)
- [adr-guard の ADR 0025: 新形式のステータス行に、実装待ちの修正、補足、注記、別のリポジトリの修正先を加える](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0025-extend-canonical-status-amendments.md)
- [adr-guard の ADR 0026: 旧形式のステータス行の修正先を、リンク先と裸の番号からも読む](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0026-read-more-legacy-references.md)
- [adr-guard の ADR 0027: 旧形式の `置換（…）` を、修正を受けた状態として読む](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0027-read-legacy-replaced-status.md)
- [adr-guard の ADR 0028: 旧形式の部分名から修飾子と区切りの記号を除き、決められない部分名を移行しない](https://github.com/tett23/adr-guard/blob/v0.13.1/docs/adr/0028-fix-legacy-part-names.md)
- [adr-guard v0.11.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.11.0)
- [adr-guard v0.12.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.12.0)
- [adr-guard v0.13.0 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.13.0)
- [adr-guard v0.13.1 のリリース](https://github.com/tett23/adr-guard/releases/tag/v0.13.1)
- [adr-guard の README（v0.13.1）](https://github.com/tett23/adr-guard/blob/v0.13.1/README.md)
