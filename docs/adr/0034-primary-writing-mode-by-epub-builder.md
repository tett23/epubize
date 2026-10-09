# ADR 0034: Kindle の primary-writing-mode は、epub-builder が頁送りの向きから書くのに任せる

ステータス: 採択

## 文脈

ADR 0029、ADR 0032、ADR 0033 では、Kindle 向けの縦書きの指定（パッケージ文書の `<meta name="primary-writing-mode" content="vertical-rl"/>`）を、epub-builder が書かないため書かないとした。
その後、epub-builder は ADR 0019 で `primary-writing-mode` を書くようになった。

- `book.toml` に `primary_writing_mode` を書けば、その値を書く。値は `horizontal-lr`、`horizontal-rl`、`vertical-lr`、`vertical-rl` のいずれかで、ほかの値は誤りになる
- 書いていなければ、`page_progression_direction` から決める。`rtl` なら `vertical-rl`、`ltr` なら `horizontal-lr` とする
- `primary_writing_mode` と `page_progression_direction` が食い違うときは誤りになる
- 古い epub-builder は、`book.toml` の知らないキーを誤りにする

epubize は、縦書きの本では `page_progression_direction = "rtl"` を書いている（ADR 0029）。そのため、いまの epub-builder で作った epubize の EPUB には、コードを変えなくても `primary-writing-mode` が入る。
kindlize の EPUB にはこの指定があり、Kindle で意図どおりの行間で表示されていた。epubize の EPUB を Kindle に送ったところ行間が狭く、その EPUB には、epub-builder が対応する前に作ったため、この指定がなかった。
ADR 0029、ADR 0032、ADR 0033 の前提が、いまの動作と食い違っている。

## 実装すること

- Kindle の `primary-writing-mode` は、epub-builder が `page_progression_direction` から決めて書くのに任せる。縦書きの本は `vertical-rl`、横書きの本は `horizontal-lr` になる
- epubize は `book.toml` に `primary_writing_mode` を書かない
- これにより、ADR 0029、ADR 0032、ADR 0033 の「`primary-writing-mode` は epub-builder が書かないため書かない」を改める。三つの ADR は、そのほかの決定が生きているため、ステータスを採択のまま残す
- 使う epub-builder は、ADR 0019 に対応したもの（コミット `48aab00` 以降）とする。README の「使う準備」と仕様（`docs/specifications.md`）に書く

## 実装しないこと

- epubize で `primary_writing_mode` を明示して書くことはしない。古い epub-builder では誤りになるため
- epub-builder の版を確かめることはしない。古い epub-builder では、これまでどおり `primary-writing-mode` のない EPUB ができる
- 右から左の横書き（`horizontal-rl`）の本は扱わない。整形の設定の組方向は縦書きと横書きの二つである

## テスト設計

- `src-tauri/src/export_tests.rs`：`book.toml` に `primary_writing_mode` を書かず、縦書きなら `page_progression_direction = "rtl"`、横書きなら `"ltr"` を書くこと（既存の、`book.toml` の中身を丸ごと比べるテストで確かめている）
- いまの epub-builder で縦書きの合成の本を作り、パッケージ文書に `<meta name="primary-writing-mode" content="vertical-rl"/>` が入ることを確かめた
- 縦書きの本を Send to Kindle で送り、行間が意図どおりになるかを、利用者が実機で確かめる。行間が狭いままなら、ほかの原因を探し、新しい ADR に記録する

## トレードオフ

- epub-builder の決め方に任せるため、epubize のコードを変えずに済み、古い epub-builder でも EPUB を作れる。その代わり、どの epub-builder を使っているかで、`primary-writing-mode` が入るかが変わる
- `primary-writing-mode` は Kindle の拡張だが、ほかのリーダーは知らない `meta` を無視し、EPUBCheck も誤りにしない（epub-builder の ADR 0019）
