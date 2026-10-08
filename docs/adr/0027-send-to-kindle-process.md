# ADR 0027: Kindle への送信は send-to-kindle を子プロセスとして起動して行い、その実行ファイルを管理画面で指定する

ステータス: 採択

## 文脈

Kindle には EPUB 3.0 を送る（ADR 0025、ADR 0026）。送信の方法はまだ決めていなかった。kindlize は SMTP でメールを送っていた。
EPUB の生成は epub-builder を子プロセスとして起動して行い、その実行ファイルは管理画面で指定する（ADR 0021、ADR 0024）。
利用者の求めで、Kindle への送信は別の実行ファイル send-to-kindle に任せ、その場所を管理画面で指定できるようにする。

## 実装すること

- Kindle への送信は、send-to-kindle を子プロセスとして起動して行う。epubize は SMTP などの送信の方法を持たない
- `settings.json` に `sendToKindlePath`（文字列、または null）を足す。null なら自動で探す
- 自動で探すときは、PATH から、それでもなければよく使う場所（`~/bin`、`~/.local/bin`、`/opt/homebrew/bin`、`/usr/local/bin`）から `send-to-kindle` という名前の実行ファイルを探す。探す手順はクローラーと epub-builder と共通にする
- 指定した実行ファイルが空、見つからない、または実行できないときは、設定を保存しない（ADR 0020 と同じ）
- 管理画面に「send-to-kindle」の節を、epub-builder と同じ形で置く。指定の欄と、いま使うものと、自動で見つかるものを表示する。送信がまだないことも書く
- send-to-kindle の扱いは `src-tauri/src/kindle.rs` に置く。いまは実行ファイルを決めるところまでを持つ

## 実装しないこと

- send-to-kindle を起動することはしない。渡すもの（EPUB のファイル、送り先など）と結果の受け取り方は、送信を実装するときに新しい ADR で決める
- 送り先のメールアドレスや SMTP の設定を epubize で持つことはしない。それらは send-to-kindle の側で扱う
- send-to-kindle を epubize に同梱しない

## テスト設計

- `src-tauri/src/kindle.rs`：指定した実行ファイルを、自動で探したものより優先すること
- `src-tauri/src/settings.rs`：実行できない send-to-kindle の指定を保存しないこと、指定を保存して読み戻せること
- `src-tauri/src/commands_tests.rs`：設定を保存すると、管理画面に出す「使用中」に反映すること
- E2E テスト：見つからないときの表示、指定して保存すると使用中に出ること、実行できないパスを拒むこと、自動で見つかるものを欄の案内に出すこと
- 実際のアプリで、管理画面に send-to-kindle の節が出ることを確かめる

## トレードオフ

- 送信を別の実行ファイルに任せるため、利用者が send-to-kindle を別に用意する必要がある。その代わり、送信の方法（SMTP、Amazon のアプリなど）を epubize と切り離して変えられ、パスワードなどの秘密を epubize で持たずに済む
- まだ使わない設定を先に置くため、画面からは指定しても何も起きない
