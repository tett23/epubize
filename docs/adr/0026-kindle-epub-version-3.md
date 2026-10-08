# ADR 0026: Kindle に送る EPUB は 3.0 で作る

ステータス: 採択

## 文脈

mobi への対応をやめ、Kindle には EPUB を送ることにした（ADR 0025）。
EPUB は epub-builder を子プロセスとして起動して作り（ADR 0021）、epub-builder は EPUB 2.0.1 と 3.0 のどちらかを選んで出せる。
Kindle が受け付ける EPUB の版について、次のことを確かめた。

- KDP のヘルプは「Kindle Publishing Guidelines の仕様に合う EPUB を受け付ける」とし、版を 2.0.1 に限っていない。リフロー型の本の文章のガイドラインは、ページ番号について EPUB 3 のアクセシビリティの指針に従うよう案内している
- Send to Kindle は 2022 年から EPUB を受け付け、Amazon が Kindle 用の形式に変換してから端末に届ける
- Kindle Publishing Guidelines の本体（PDF）は読めず、EPUB 3 のどの機能に対応するかは確かめていない

epubize で作る本は日本語の縦書きの小説が主になる。右から左への頁送り（`page-progression-direction`）は EPUB 3 にしかなく、epub-builder も EPUB 2.0.1 ではこれを書かない（epub-builder の ADR 0003）。

## 実装すること

- Kindle に送る EPUB は、epub-builder に版として `3.0` を指定して作る
- send to Kindle のボタン（ADR 0025）は、この版で作った EPUB を送る

## 実装しないこと

- Kindle に送るために EPUB 2.0.1 を作ることはしない
- この ADR では、EPUB の生成と Kindle への送信は実装しない
- download epub で取り出す EPUB の版は、この ADR では決めない。EPUB の生成を実装するときに決める

## テスト設計

- 方針の決定であり、この時点ではコードを伴わないため、テストはない
- 送信を実装するときに、Kindle に送る EPUB を作るとき epub-builder に `3.0` を渡すことをテストする
- 送信を実装した後に、Send to Kindle で実際の端末に送り、縦書きと右から左への頁送り、ルビ、傍点が表示されることを確かめる。表示が崩れるときは、EPUB 2.0.1 と見比べて、この ADR を見直す

## トレードオフ

- EPUB 3 にしか対応しない機能を使えるため、縦書きの頁送りの向きを正しく伝えられる。その代わり、EPUB 3 に対応しない古い端末やアプリでは読めないことがある
- Kindle が EPUB 3 のどの機能に対応するかを確かめていないため、送った後の表示は実機で確かめるまで分からない
