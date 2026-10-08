# ADR 0021: EPUB は epub-builder を子プロセスとして起動して作る

ステータス: 採択

## 文脈

EPUB のファイルを作る処理は、別の公開リポジトリ [epub-builder](https://github.com/tett23/epub-builder) で、Deno で動く TypeScript のライブラリとして作っている。
epubize の取り込みと保存は Rust の側にあり（ADR 0018）、作品の本文と挿絵は SQLite にある（ADR 0017）。
epub-builder は実装の途中で、epubize から渡す形（インターフェース）はまだ決まっていない。

## 実装すること

- 作品や話の EPUB を作るときは、epub-builder を子プロセスとして起動する。クローラー（ADR 0004）と同じく、Rust の側から起動する
- epub-builder は epubize に同梱しない。実行ファイル（または Deno で動かすための入口）の場所は、管理画面（ADR 0020）で指定できるようにする
- 渡すデータの形、出力の受け取り方、エラーの伝え方は、epub-builder のインターフェースが決まったときに、新しい ADR で決める

## 実装しないこと

- epub-builder を Rust で作り直すことはしない
- epub-builder を WebView の中で動かすことはしない。データベースの読み出しと子プロセスの起動は Rust の側に寄せる
- 整形の設定を本文に適用するか（ADR 0011、ADR 0017）は、epub-builder の仕様とあわせて、このとき決める。この ADR では決めない

## テスト設計

- 方針の決定であり、この時点ではコードを伴わないため、テストはない
- 実装するときは、偽の epub-builder を子プロセスとして使い、渡すデータと、出力とエラーの受け取りをテストする

## トレードオフ

- 子プロセスにするため、EPUB を作るたびにプロセスを起動する。その代わり、epub-builder を単独で使え、epubize と別々に直せる
- 利用者が epub-builder を別に用意する必要がある
