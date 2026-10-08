# ADR 0032: kindlegen と striptool で MOBI を作る download mobi を置き、送信は EPUB のままにする

ステータス: 採択

## 文脈

ADR 0025 では mobi への対応をやめ、Kindle には EPUB を送ることにした。ADR 0026 で、Kindle に送る EPUB は 3.0 とした。
Kindle の端末に USB などで直接入れるには、MOBI が要る。Kindle Previewer の `Contents/Resources/KFXGen/bin` には、kindlegen と striptool が入っている。

- kindlegen（V2.9）は、EPUB から MOBI（KF8 を含む）を作る。`-o` にはファイルの名前だけを渡せ、入力と同じディレクトリに書く。警告があると終了コード 1、誤りがあると 2 で終わる。表紙がないと警告（W14016）を出す
- kindlegen は、作った MOBI に元の EPUB（SRCS の記録）を埋め込む。そのぶん大きくなる
- striptool（Amazon striptool 1.0）は、引数に MOBI のパスを一つ取る。入力と同じディレクトリに数字の名前のディレクトリを作り、元の EPUB を取り除いた同じ名前の MOBI と、取り出した EPUB を書く。成功すると終了コード 0、ファイルがないときや引数の誤りでは 255 で終わる。使い方は表示しないため、試して確かめた

Send to Kindle のメールは、2022 年から MOBI と AZW3 を受け付けないと考えられる。そのため利用者は、送信は EPUB のままにし、MOBI は取り出して端末に入れることにした。

## 実装すること

- 作品のページ、作品のページの話の行、話のページに、download mobi のボタンを置く。download epub と同じく、本文を取得済みの話だけに置き、範囲（作品か話）は ADR 0029 と同じとする
- download mobi は、ADR 0029 と同じ EPUB 3.0 を作り、kindlegen で MOBI にし、striptool で元の EPUB を取り除き、ダウンロードのディレクトリに `<題名>.mobi` で書く。同じ名前があれば番号を付ける
- kindlegen と striptool は EPUB の隣に書くため、EPUB を作業用の空のディレクトリに置いてから起動する。kindlegen には `-locale ja` を渡し、誤りを日本語で受け取る
- kindlegen の終了コードが 0 か 1 で、MOBI ができていれば成功とする。それ以外は、標準出力のうち誤りの行を画面に出す
- striptool が 0 以外で終わったとき、または元の EPUB を取り除いた MOBI がないときは、誤りとする
- `settings.json` に `kindlegenPath` と `striptoolPath` を足す。null なら、クローラーと同じ場所から `kindlegen` と `striptool` を自動で探す。実行できない指定は保存しない（ADR 0020）
- 管理画面に kindlegen と striptool の節を、epub-builder と同じ形で置く
- これにより、ADR 0025 の「mobi への対応をやめる」を、端末に直接入れるための download mobi について改める。ADR 0025 は、そのほかの決定が生きているため、ステータスを採択のまま残す

## 実装しないこと

- send to Kindle で MOBI を送ることはしない。送信は ADR 0026 のとおり EPUB 3.0 のままとする
- kindlegen と striptool を epubize に同梱しない。Kindle Previewer から利用者が取り出して置く
- kindlegen の `-c1`、`-c2` などの圧縮の指定、`-dont_append_source` は使わない
- 表紙を作って kindlegen の警告をなくすことはしない
- Kindle 向けの縦書きの指定（パッケージ文書の `primary-writing-mode`）は書かない。epub-builder が書かないため（ADR 0029）

## テスト設計

- `src-tauri/src/mobi.rs`：指定した実行ファイルを自動で探したものより優先すること。偽の kindlegen と striptool（sh のスクリプト）で、渡す引数、終了コード 1 を成功とすること、数字のディレクトリから MOBI を拾うこと、誤りの伝え方（kindlegen の誤りの行、MOBI がないとき、起動できないとき、striptool の失敗）
- `src-tauri/src/settings.rs`：実行できない kindlegen と striptool の指定を保存しないこと、指定を保存して読み戻せること
- `src-tauri/src/commands_tests.rs`：偽の epub-builder、kindlegen、striptool で、ダウンロードのディレクトリに元の EPUB を取り除いた MOBI を書くこと、striptool がないときの誤り
- E2E テスト：作品、話の行、話のページの download mobi が範囲を渡すこと、本文のない話には出さないこと、管理画面の二つの節の表示と保存と拒否
- 開発用のデータの作品から、本物の epub-builder、kindlegen、striptool で MOBI ができ、元の EPUB を取り除いて小さくなることを確かめた（122 話で 9.3MB から 7.4MB）
- 端末での表示は、利用者が MOBI を端末に入れて確かめる

## トレードオフ

- kindlegen は Amazon が配布をやめた古いツールで、x86_64 だけのため Apple Silicon では Rosetta が要る。その代わり、EPUB を手で変換せずに MOBI を取り出せる
- striptool の使い方は公開されておらず、試して確かめた動きに頼る。出力のディレクトリの名前の決まり方は分からないため、名前ではなく、中に同じ名前の MOBI があるディレクトリを探す
- 縦書きの指定を CSS と頁送りの向きだけで伝えるため、kindlegen が縦書きの本として扱うかは、端末で確かめるまで分からない
