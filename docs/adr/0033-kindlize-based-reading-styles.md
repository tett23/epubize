# ADR 0033: EPUB の行間、禁則、余白、ルビの大きさを kindlize のスタイルにならって書く

ステータス: 採択

## 文脈

EPUB のスタイルシートには、縦書きの指定、段落の余白（ADR 0030）、挿絵の大きさ、扉と目次と奥付（ADR 0031）、見出しの大きさだけを書いていた。
利用者が Kindle で確かめたところ、行間が狭すぎた。行間を指定しておらず、リーダーの既定の行間で組まれるためである。
以前作った [kindlize](https://github.com/tett23/kindlize) の EPUB のスタイル（`packages/epub/src/assets/stylesheet.ts`、`pageCss.ts`）は、Kindle で読むために調整したもので、利用者はこれを参考にするよう求めた。

## 実装すること

- 段落（`p`）と改行（`br`）の行の高さを `1.7em` にする
- `html` と `body` の禁則を厳しくする（`line-break: strict`、`-webkit-` と `-epub-` の付いたものも書く）
- 本文は両端をそろえる（`text-align: justify`）。本文の余白は、縦書きなら左右に 5pt、横書きなら上下に 3pt とする
- 段落は `word-break: break-all` とし、内側の余白をなくす。段落の外側の余白は、ADR 0030 のとおり removeEmptyLine が有効なときだけなくす
- 区切り線（`hr`）の余白を `0 2em` とする
- ルビ（`ruby > rt`）の文字の大きさを `0.33em` とする
- ページの上下の余白（`@page`）を 5pt とする
- 縦書きの指定は、`html` と `body` の両方に書く

## 実装しないこと

- kindlize の `.ih`（句読点で始まる行の字下げ）は書かない。クローラーが付けないクラスのため
- ルビの大きさの `!important` は付けない
- 行間を整形の設定で変えられるようにはしない
- kindlize の OPF の `primary-writing-mode` は、epub-builder が書かないため書かない（ADR 0029、ADR 0032）

## テスト設計

- `src-tauri/src/export_tests.rs`：縦書きと横書きで、行の高さ、禁則、本文の余白、ルビの大きさ、ページの余白を書くこと。removeEmptyLine が無効なら段落の外側の余白を書かないこと
- 端末での見え方は、利用者が Kindle で確かめる

## トレードオフ

- 行の高さを `em` で決めるため、リーダーの行間の設定より、このスタイルが優先されることがある。その代わり、Kindle で行間が狭すぎることがなくなる
- `word-break: break-all` のため、英単語の途中でも折り返す。縦書きの日本語の本では、行末がそろうことを優先する
