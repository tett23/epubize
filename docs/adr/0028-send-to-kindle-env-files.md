# ADR 0028: send-to-kindle の .env と .env.example は既定でアプリ用データ領域に置き、管理画面で場所を変えられるようにし、.env に足りないキーを表示する

ステータス: 採択

## 文脈

Kindle への送信は send-to-kindle を子プロセスとして起動して行う（ADR 0027）。
send-to-kindle は、送信元と送り先のメールアドレス、SMTP の設定を、`-e`（`--env-file`）で渡したファイルから読む。渡さなければカレントディレクトリの `.env` から、それもなければ環境変数から読む。`-e` で渡したファイルがなければ誤りにする（send-to-kindle のある dotfiles の ADR 0022）。
`.env` にはパスワードを書くため、epubize の画面に値を出したくない。一方で、設定が足りないことは送る前に分かりたい。
利用者の求めで、`.env` と `.env.example` のパスを管理画面で指定できるようにする。

## 実装すること

- `settings.json` に `sendToKindleEnvPath` と `sendToKindleEnvExamplePath`（文字列、または null）を足す
- null なら、環境ごとのアプリ用データ領域（データベースと同じ `~/Library/Application Support/com.github.tett23.epubize/<環境>/`）の `.env` と `.env.example` を使う
- send-to-kindle を起動するときは、`.env` を `-e` で渡す。ただし、指定がなく、既定の場所にも `.env` がなければ `-e` を渡さない（send-to-kindle は環境変数から読む）
- `.env` と `.env.example` の指定は、ファイルとしてあるときだけ保存する。名前は問わない。空の指定も保存しない（ADR 0020 と同じ）
- `.env.example` があれば、`.env.example` にあるキーのうち、`.env` にないか値が空のものを管理画面に出す。キーがそろっていれば、そう出す。`.env` がないとき、どちらかを読めないときは、その理由を出す。`.env.example` がなければ何も出さない
- 管理画面には値を出さず、キーの名前だけを出す
- dotenv の読み方は、`KEY=VALUE` の行だけを見る。空行と `#` で始まる行は飛ばし、前の `export ` と、値を囲む `"` または `'` を外す
- 管理画面では send-to-kindle の節に、二つの欄と、いま使う場所と、検査の結果を置く。欄の案内には既定の場所を出す

## 実装しないこと

- この ADR では send-to-kindle を起動しない。起動は Kindle への送信を実装するときに行う
- `.env` の値を epubize で読んで send-to-kindle に渡すことはしない。`.env` は send-to-kindle が読む
- 既定の場所に `.env` や `.env.example` を作ることはしない。利用者が置く
- `.env` を管理画面で編集したり、`.env.example` から `.env` を作ったりはしない
- 複数行にわたる値や変数の展開など、dotenv の細かい書き方は扱わない
- 検査は保存した設定について行い、入力中の欄については行わない

## テスト設計

- `src-tauri/src/kindle.rs`：`.env.example` の順に、ないキーと値が空のキーを出し、`export ` と引用符と重複を扱うこと。指定がなければアプリ用データ領域のファイルを使い、指定があればそれを使うこと。`.env.example` がなければ検査せず、`.env` がないとき、足りないとき、そろったときを区別すること
- `src-tauri/src/settings.rs`：ないファイル、空の指定、ファイルでない `.env.example` を保存しないこと。`.env` という名前でないファイルの指定を保存して読み戻せること
- `src-tauri/src/commands_tests.rs`：管理画面に既定の場所を出すこと、設定を保存すると検査の結果に反映すること
- E2E テスト：既定の場所を欄の案内に出して足りないキーを出すこと、`.env.example` がなければ結果を出さないこと、指定して保存すると使用中に出すこと、見つからない `.env` を拒むこと
- 実際のアプリで、send-to-kindle の節に二つの欄と既定の場所が出ることを確かめる

## トレードオフ

- `.env.example` との比較なので、`.env.example` にないキーが足りなくても気づけない。その代わり、send-to-kindle の要るキーを epubize に書かずに済み、send-to-kindle の変更に追いつく必要がない
- dotenv を簡単な読み方で読むため、send-to-kindle が読める書き方でも、キーが足りないと誤って出すことがある
- 既定の場所を環境ごとに分けるため、development と production で別の送り先や SMTP の設定を使える。その代わり、両方で使うときは二か所に置く必要がある
