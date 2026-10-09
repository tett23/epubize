# epubize の仕様

この文書は、現在の実装が満たす仕様をまとめたものである。
決定の理由は ADR（[docs/adr/](adr/)）に記録しており、この文書では繰り返さない。
この文書と ADR が食い違う場合は、ADR を正とする。ADR によって仕様が変わったときは、この文書と README を同じ変更で直す。

## 目次

1. 概要
2. 構成
3. 環境とデータの置き場所
4. 購読の一覧
5. データベース
6. クローラーとの受け渡し
7. 取得
8. 定期取得
9. 画面
10. 管理画面と設定
11. 書き出し
12. EPUB、zip、MOBI の書き出しと Kindle への送信
13. 外部のプログラムの探し方と起動
14. 開発
15. 扱わないこと
16. ADR の状態

## 1. 概要

epubize は、Web 小説を取得して SQLite に持ち、EPUB にして書き出したり Kindle に送ったりする、macOS 向けのデスクトップアプリである（ADR 0001、ADR 0003）。
取得、EPUB の生成、Kindle への送信は、それぞれ別のプログラムを子プロセスとして起動して行う。

| プログラム       | 役割                                       | リポジトリ                                                           |
| ---------------- | ------------------------------------------ | -------------------------------------------------------------------- |
| `novel-crawler`  | Web 小説サイトから作品、目次、本文、挿絵を取得する | 非公開                                                               |
| `epub-builder`   | ディレクトリから EPUB を作る               | [tett23/epub-builder](https://github.com/tett23/epub-builder)        |
| `send-to-kindle` | ファイルをメールで Kindle に送る           | [tett23/send-to-kindle](https://github.com/tett23/send-to-kindle)    |
| `kindlegen`      | EPUB を MOBI にする                        | Kindle Previewer に入っているもの                                    |
| `striptool`      | MOBI に埋め込まれた元の EPUB を取り除く    | Kindle Previewer に入っているもの                                    |

これらは epubize に同梱しない（ADR 0004、ADR 0021、ADR 0027、ADR 0032）。

## 2. 構成

- GUI は Tauri 2 で作る。画面は React と TypeScript で書き、Vite でビルドする。スタイルは Tailwind CSS v4 で書く（ADR 0003、ADR 0006、ADR 0008）
- パッケージの管理は pnpm で行う。アプリの識別子は `com.github.tett23.epubize` とする
- データの読み書き、取得のキュー、子プロセスの起動は Rust の側（`src-tauri/`）で行う。画面は Tauri のコマンド（`invoke`）で Rust の側を呼ぶ
- 1 回の取得が終わるたびに、Rust の側はイベント `fetch-done` を画面に送る。定期取得を行ったときはイベント `scheduled-fetch` を送る
- 外部のサイトへのリンクは、`tauri-plugin-opener` で既定のブラウザに開く。アプリのウィンドウの中では開かない
- 画面の切り替えは react-router の `MemoryRouter` で行う

データの流れの図は README の「仕組み」にある。

## 3. 環境とデータの置き場所

環境は `development` と `production` の二つである（ADR 0014）。
環境変数 `EPUBIZE_ENV` があればそれに従い、なければ debug ビルドを `development`、release ビルドを `production` とする。ほかの値はエラーにする。
画面の見出しの横に環境を表示する。

| データ                         | 置き場所                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------ |
| 購読の一覧                     | `~/.config/epubize/<環境>/novels.json`（`XDG_CONFIG_HOME` があれば `~/.config` の代わりに使う） |
| 設定                           | `~/.config/epubize/<環境>/settings.json`                                                   |
| SQLite                         | `~/Library/Application Support/com.github.tett23.epubize/<環境>/epubize.sqlite3`           |
| send-to-kindle の `.env` の既定 | SQLite と同じディレクトリの `.env`                                                         |
| `.env.example` の既定          | SQLite と同じディレクトリの `.env.example`                                                 |
| ウィンドウの位置と大きさ       | SQLite と同じディレクトリの `window-state.json`（macOS ではアプリ用設定領域とアプリ用データ領域が同じ場所になる） |
| 書き出したファイル             | ダウンロードのディレクトリ（macOS では `~/Downloads`）。なければホームディレクトリ         |

### ウィンドウの位置と大きさ

- 移動と大きさの変更のたびに値を覚え、ウィンドウを閉じるときとアプリの終了時に `window-state.json` に書く（ADR 0007）
- 最大化、最小化、全画面のときの位置と大きさは記録しない
- 起動時に、記録したウィンドウの全体がいずれかのディスプレイの上にあれば、位置と大きさを戻す。一部でも載らなければ、既定（800×600、画面の中央）のままにする
- ウィンドウは隠した状態で作り、位置を戻してから表示する

## 4. 購読の一覧

- 購読している作品は `novels.json` に、kindlize と同じ形で持つ（ADR 0014）。データベースには持たない

```json
{ "subscribe": { "narou": ["n0000aa"], "novel18": [], "hameln": [100000], "kakuyomu": ["1000"] } }
```

- サイトは `narou`、`novel18`、`hameln`、`kakuyomu` の四つである。hameln の作品 ID は数値、ほかは文字列とする
- `subscribe` 以外のキーは読まず、書き換えるときもそのまま残す。キーの順も保つ
- 作品の URL（作品の中の話の URL でもよい）から、サイトと作品 ID を取り出して加える
- 書き換えるときは、元のファイルを `novels.json.bak` に写し、一時ファイル `novels.json.tmp` に書いてから置き換える。すでに購読している作品を加えるときは、どちらのファイルにも触れない
- JSON として読めない、または形が違うときはエラーにし、ファイルに触れない。ファイルがなければディレクトリごと作る
- 購読を外しても、取得済みの作品のデータは消さない（ADR 0020）

## 5. データベース

### マイグレーション

- SQLite は rusqlite（`bundled`）で扱う（ADR 0013）
- マイグレーションは `src-tauri/migrations/NNNN_name.sql` に置き、`build.rs` がアプリに埋め込む。版は 1 からの連番で、違えばビルドを止める
- 適用済みの版は `PRAGMA user_version` に持つ。1 つのマイグレーションを 1 つのトランザクションで適用する
- データベースの版がアプリの知っている版より新しいときは、エラーにする
- 接続ごとに `foreign_keys` を有効にし、`journal_mode` を WAL にする
- アプリは起動時に、ファイルがなければ作り、未適用のマイグレーションを全て適用する。失敗したら起動しない
- CLI `src-tauri/src/bin/migrate.rs` と、mise のタスク `db:migrate`、`db:status`、`db:new`、`db:reset` で操作できる。パスは `--db`、環境変数 `EPUBIZE_DB`、既定の場所の順に決める

### スキーマ

マイグレーションは `0001_create_tables.sql` の一つである（ADR 0017）。表は `STRICT` で作り、日時は UTC の ISO 8601 の文字列で持つ。

| 表               | 持つもの                                                                                                                                                                                                  |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `novels`         | `site` と `site_id`（組で一意）、`url`、`title`、`author_name`、`author_url`、`description`、`episode_count`、`is_concluded`（不明なら null）、`metadata_fetched_at`、`normalize_options`（JSON、null なら既定） |
| `episodes`       | `novel_id`、`no`（目次の上での 1 始まりの位置）、`url`（一意）、`title`、`chapter`（null 可）、`published_at`、`revised_at`、`char_count`、`body_fetched_at`（null なら未取得）、`body_revised_at`、`sent_at`、`preface`、`body`、`afterword` |
| `episode_images` | `episode_id`、`position`（話の中で最初に現れた順）、`url`                                                                                                                                                 |
| `images`         | `url`（主キー）、`content_type`、`data`（BLOB）、`byte_length`、`fetched_at`                                                                                                                             |

- 作品を消すと話が、話を消すと話と挿絵の対応が消える。挿絵は複数の話から参照されても一つだけ持つ
- 章は表に分けず、続く話で同じ `chapter` の名前なら同じ章とみなす
- 各表に `created_at` と `updated_at` を持つ

## 6. クローラーとの受け渡し

取り決めは epubize の側が持ち、JSON Schema は [schema/crawler-output.v1.schema.json](../schema/crawler-output.v1.schema.json) にある（ADR 0005、ADR 0009）。README の「クローラーとのインターフェース」に詳しい。

- コマンドは `toc <作品の URL>`、`episode <話の URL>`、`image <画像の URL>` の三つで、1 回が 1 回の取得にあたる
- 標準出力に 1 行 1 レコードの JSON（JSON Lines）を書く。レコードは `novel`、`toc_entry`、`episode`、`image`、`error` で、版 `v` は 1 である
- 失敗したときは `error` レコードを 1 行書き、0 以外の終了コードで終わる。`error` レコードがなければ、標準エラー出力の内容を失敗の理由とする
- 本文は、縦書き向けに正規化した CommonMark で受け取る。元の 1 行を 1 段落とし、空行は `<br>` だけの段落、ルビは `<ruby>`、傍点は `text-emphasis` の `<span>` で表す。epubize は同じ正規化を重ねない

### 取り込み

- 標準出力を 1 行ずつ JSON Schema（形式の検査を含む）で確かめてから読む。合わない行が一つでもあれば、どの行かを示して保存しない（ADR 0018）
- 日時は UTC に揃えて保存する
- 目次は、作品を `site` と `site_id` で、話を URL で対応させて書き換える。目次から消えた話は消す。目次の順が変わっても、取得済みの本文は残る
- 話は、本文と、本文を取得したときの改稿日時を書き、話と挿絵の対応を入れ直す
- 挿絵は、base64 を戻した長さが `byteLength` と合わなければ保存しない

## 7. 取得

### キュー

- キューは Rust（tokio）で持ち、取得する URL のホスト名ごとに 1 本とする（ADR 0015）
- 同じホストの取得は順に、違うホストの取得は並列に行う。取得のたびに、取得と 5 秒の待ちを 1 組として末尾に積む
- 失敗の種類（`error` の `kind`）で積み直すかを決める

| `kind`                                                                   | 扱い                                                                       |
| ------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| `server_error`、`network`                                                | 末尾に積み直す。積み直しは 3 回まで                                        |
| `rate_limited`                                                           | `retryAfter` の日時まで（なければ 60 秒）待つタスクを先頭に積み、その後ろで取り直す。回数に数えない |
| `unsupported_url`、`not_found`、`forbidden`、`http_error`、`parse_error` | 積み直さない                                                               |

- クローラーが見つからなくてもキューは作り、取得は「クローラーがない」という失敗にする（ADR 0020）
- 取得が終わるたび（成功、またはあきらめたとき）に、`fetch-done` でコマンド、URL、失敗の理由を画面に送る（ADR 0016）

### 続く取得

取得の要求に「続く取得を積むか」を持たせ、結果を保存した後に積む（ADR 0018）。

- 目次の後には、未取得か、目次の改稿日時が本文を取得したときより新しい話の本文を積む
- 話の後には、まだ持っていない挿絵を積む

### ボタン

| ボタン             | 働き                                                                                                             |
| ------------------ | ---------------------------------------------------------------------------------------------------------------- |
| fetch all          | 全てのキューを破棄し、購読の全作品の目次を、続く取得を積む形で積む。各ホストの先頭に 5 秒の待ちを一つ置く      |
| fetch all metadata | 全てのキューを破棄し、購読の全作品の目次だけを積む                                                               |
| add（未取得）      | その作品の目次だけを積む                                                                                         |
| fetch（作品）      | その作品の目次を、続く取得を積む形で積む                                                                         |
| refetch（話）      | その話の本文を、続く取得を積む形で積む                                                                           |
| remove episodes    | 確かめた後、その作品の話を全て消す。作品と挿絵は残す                                                             |

## 8. 定期取得

- アプリが動いている間、毎日決まった時刻（既定は 3:00、ローカル時刻）に fetch all を行う（ADR 0019、ADR 0020）
- 壁時計を 1 分ごとに見て、前に見たときから時刻を越えていれば行う。スリープで過ぎていた場合は起きた後に 1 回だけ行う。時計が戻ったとき、起動した時点、無効のとき、クローラーが見つからないときは行わない
- 行ったときは `scheduled-fetch` で、積んだ作品の数か失敗の理由を画面に知らせる

## 9. 画面

画面は全てデータベースから読み、`fetch-done` を受けるたびに読み直す。続けて届いたときは 0.5 秒まとめる（ADR 0018）。
一覧と目次では本文を読まず、本文は話のページでだけ読む。

### トップ（`/`）

- 作品の URL を入れて購読に加える欄（ADR 0014）
- fetch all、fetch all metadata のボタン、キューに残っているタスクの数（1 秒ごとに読む）、定期取得の時刻と行ったこと
- 取得の結果の一覧（新しい順に 20 件）
- 作品数、話数、未取得の話数と、1 回の取得を 5 秒とした取得時間の目安
- 作品の一覧（最新話の日時の新しい順）。最新話の日時、作品 ID、作者、題名、取得済み / 全話（全話を取得していなければ赤）
- 購読しているが作品として加えていないもの（未取得）と、add のボタン

### latest（`/episodes/latest`）

- 全作品の話を公開日時の新しい順に 100 件、公開日ごとに区切って並べる（ADR 0011）
- 話ごとに refetch と、本文を取得済みなら send to Kindle を置く。送った話には `(sent)` を付ける

### 作品（`/novels/:novelId`）

- 題名、作者、あらすじ、更新日、文字数、完結済みか
- fetch、remove episodes、download zip、download epub、download mobi、send to Kindle のボタン。send to Kindle は、本文を取得済みの話の数を示して確かめてから送る
- 整形の設定
- 章ごとの目次。公開日時、話の番号、題名（本文を取得済みなら話のページへのリンク）、送った印を並べ、本文を取得済みの話には題名の後ろに download epub、download mobi、send to Kindle を置く（ADR 0025）

### 話（`/novels/:novelId/:episodeId`）

- 作品の題名、話の番号と題名、送った印
- refetch のボタンと、本文を取得済みなら download zip、download epub、download mobi、send to Kindle
- 更新日、前後の話へのリンク、整形の設定
- 本文のプレビューと、元の Markdown

### プレビュー

- 本文の Markdown は、話の題名を見出しにし、前書き、本文、後書きを区切り線でつないで表示する
- marked で HTML にし、DOMPurify で許した要素と属性（ルビ、傍点の `style`、強調など）だけを残す（ADR 0011）
- 挿絵は、取得済みのものをデータベースから data URL で読む。取得していない挿絵は `src` を外し、代替テキストだけを残す（ADR 0018）
- 整形の設定の組方向で縦書きと横書きを切り替える。removeEmptyLine が無効なら、段落の前後に 1 行分の余白を置く（ADR 0030）

### 整形の設定

- 項目は kindlize と同じ（`direction` と kindlize の整形の項目）で、作品ごとにデータベースに JSON で持つ。変えるたびに保存し、保存を待たずに画面に反映する（ADR 0017、ADR 0018）
- 型の合わない項目は既定の値として読む
- 本と画面に使うのは、組方向（`direction`）と removeEmptyLine だけである（ADR 0029、ADR 0030）

### 確認と結果の表示

- 取り消せない操作は、ブラウザのダイアログではなく、ボタンの横に確認の文と「実行する」「やめる」を出して確かめる。確認の表示は `alertdialog` とする（ADR 0023）
- 処理中はボタンを押せなくする。失敗したら理由をボタンの横に出す
- 書き出したときはファイルの名前と「Finder で表示」を、送ったときは送った話の数をボタンの横に出す（ADR 0029）

## 10. 管理画面と設定

### settings.json

```json
{
  "crawlerPath": null,
  "epubBuilderPath": null,
  "sendToKindlePath": null,
  "sendToKindleEnvPath": null,
  "sendToKindleEnvExamplePath": null,
  "kindlegenPath": null,
  "striptoolPath": null,
  "schedule": { "enabled": true, "at": "03:00" }
}
```

- 実行ファイルの項目が null なら、自動で探す（13 節）。`.env` と `.env.example` が null なら、既定の置き場所（3 節）のものを使う
- ファイルがなければ既定値を使う。知らない項目は無視し、ない項目は既定値で埋める（ADR 0020）
- 保存する前に確かめ、次のときは保存しない
  - 時刻が `HH:MM` でない
  - 実行ファイルの指定が空、見つからない、または実行できない
  - `.env` と `.env.example` の指定が空、またはファイルとしてない
- 書き換える前に `settings.json.bak` を作り、一時ファイルに書いてから置き換える
- ファイルが壊れていても既定値で起動し、理由を管理画面に出す
- 保存した設定は再起動せずに反映する。クローラーは、積んであるタスクが取り出されたときから新しいものを使う。定期取得は、壁時計を見るたびに設定を読み直す

### 管理画面（`/settings`）

上から次の節を置く。

- クローラー、epub-builder、send-to-kindle、kindlegen、striptool：実行ファイルの指定の欄と、いま使うもの、自動で見つかるもの（ADR 0020、ADR 0024、ADR 0027、ADR 0032）
- send-to-kindle の節には、`.env` と `.env.example` の指定の欄と、いま使う場所も置く。`.env.example` があれば、`.env.example` にあるキーのうち `.env` にないか値が空のものを出す。値は出さない。`.env` がないとき、読めないときは理由を出す（ADR 0028）
- 定期取得：行うかと時刻
- 保存のボタン
- 購読：購読の一覧（作品として加えたものは題名も）と、確かめてから外すボタン
- データの置き場所：環境、購読の一覧、設定、データベースのパス

`.env` の検査で読む dotenv は、`KEY=VALUE` の行だけを見る。空行と `#` の行を飛ばし、前の `export ` と値を囲む引用符を外す。

## 11. 書き出し

download epub、download zip、download mobi、send to Kindle は、同じ手順で epub-builder のプロジェクトのディレクトリを作る（ADR 0029、ADR 0031）。

### 範囲

- 作品なら本文を取得済みの話の全てを 1 冊に、話ならその話だけを 1 冊にする。本文を取得した話がなければ誤りとする

### プロジェクトのディレクトリ

一時ディレクトリに次を書く。

```
book.toml
meta/titlepage.xhtml        本の扉（題名、著者）
meta/colophon.xhtml         奥付
body/0000.xhtml             目次のページ（二話以上の本だけ）
body/0001.md                章のない話
body/0002-<章の名前>/       続く話で同じ章の名前を持つものをまとめる
  index.xhtml               章の扉（章の名前）
  0001.md
assets/style.css
assets/images/0001.png      取得済みの挿絵
```

- 連番の桁は、同じディレクトリの中でそろえる（4 桁以上）。章の名前は、`/` と `:` を全角にし、制御文字を除き、200 バイトまでに切る
- 一話だけの本では章にまとめず、目次のページも置かない

### book.toml

| キー                         | 値                                                                          |
| ---------------------------- | --------------------------------------------------------------------------- |
| `identifier`                 | `epubize:<サイト>:<作品 ID>`。話だけの本では末尾に `:<話の番号>`             |
| `title`                      | 作品の題名。話だけの本では「作品の題名 話の題名」                           |
| `language`                   | `ja`                                                                        |
| `authors`                    | 作者。空なら書かない                                                        |
| `description`                | あらすじ。空なら書かない                                                    |
| `page_progression_direction` | 縦書きなら `rtl`、横書きなら `ltr`                                          |

`primary_writing_mode` は書かない。epub-builder は `page_progression_direction` から Kindle の `primary-writing-mode` を決め、パッケージ文書に `<meta name="primary-writing-mode" content="…"/>` として書く。縦書きの本は `vertical-rl`、横書きの本は `horizontal-lr` になる（ADR 0034、epub-builder の ADR 0019）。
epub-builder の ADR 0019 に対応する前の epub-builder では、この指定のない EPUB ができる。

### 話の文書

- 話の題名を `#` の見出しにし、前書き、本文、後書きを区切り線（`---`）でつなぐ
- 題名の ASCII の記号と `《` には `\` を前に置き、Markdown とルビの記法と取られないようにする
- 本文の `《` と `[^` には `\` を前に置き、epub-builder のルビと脚注の記法と取られないようにする（ADR 0029）
- 挿絵（`![代替テキスト](URL)`）は、取得済みで、epub-builder が扱える形式（JPEG、PNG、GIF、SVG）のものを、最初に現れた順の番号で `assets/images/` に置き、参照を書き換える。取得していない挿絵と扱えない形式の挿絵は、代替テキストに置き換える

### 扉、目次、奥付

- 本の扉は題名と著者を置く。著者が空なら置かない
- 章の扉は章の名前を置く。話の単位と、章のない話には置かない
- 目次のページは、話の題名を並べ、章は章の扉を指す項目の下に入れ子にする。リンクは本文のファイルの元の名前をパーセントエンコードして書き、epub-builder が書き換える
- 奥付は、題名、著者、掲載しているサイトの名前と URL（話だけの本なら話の URL）、収録した話、作った日（ローカル時刻）と epubize を置く

### スタイルシート

- 縦書きなら `html` と `body` に `writing-mode: vertical-rl`（`-webkit-` と `-epub-` の付いたものも）を書く
- 段落と改行の行の高さは `1.7em`、禁則は `strict`、本文は両端そろえとする。本文の余白は、縦書きなら左右に 5pt、横書きなら上下に 3pt、ページの上下の余白は 5pt とする。段落は `word-break: break-all` とする。区切り線の余白は `0 2em`、ルビの大きさは `0.33em` とする（ADR 0033）
- removeEmptyLine が有効なら段落の外側の余白をなくす。無効なら指定せず、リーダーの既定に任せる（ADR 0030）
- 話の題名の見出しは `1.2em`、本と章の扉の題名は `1.6em` とする
- 挿絵は頁に収まるようにする

## 12. EPUB、zip、MOBI の書き出しと Kindle への送信

ファイルの名前は題名から作り、`/` と `:` を全角にし、制御文字と先頭の `.` を除く。ダウンロードのディレクトリに同じ名前があれば、`<題名> (2).epub` のように番号を付ける。

| 操作           | 働き                                                                                                                                                                   |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| download epub  | `epub-builder build <プロジェクト> --epub-version 3.0 --quiet --output <ファイル>` で EPUB 3.0 を作り、`<題名>.epub` で書く（ADR 0026、ADR 0029）                     |
| download zip   | プロジェクトのディレクトリを、題名のディレクトリに入れた zip にして `<題名>.zip` で書く。epub-builder は起動しない                                                     |
| download mobi  | EPUB 3.0 を作業用の空のディレクトリに作り、kindlegen で MOBI にし、striptool で元の EPUB を取り除いて `<題名>.mobi` で書く（ADR 0032）                                  |
| send to Kindle | EPUB 3.0 を `<題名>.epub` の名前で作り、send-to-kindle に渡す。送れたら送った話の全てに `sent_at` を記録し、送れなければ記録しない（ADR 0027、ADR 0029）               |

### kindlegen と striptool

- kindlegen は `<EPUB> -locale ja -o <名前>.mobi` で起動し、EPUB と同じディレクトリに書かせる。終了コードが 0 か 1（警告）で MOBI ができていれば成功とし、それ以外は標準出力の誤りの行を出す
- striptool は `<MOBI のパス>` で起動する。MOBI と同じディレクトリに数字の名前のディレクトリを作り、そこに元の EPUB を取り除いた MOBI を書くので、それを拾う。0 以外で終わるか、MOBI がなければ誤りとする

### send-to-kindle

- `send-to-kindle [--env-file <.env>] <ファイル>` で起動する
- `.env` は、指定があればそれを、なければ既定の置き場所にあればそれを `--env-file` で渡す。どちらもなければ渡さず、send-to-kindle は自分の探し方（環境変数、カレントディレクトリの `.env`、`~/.config/send-to-kindle/.env`）で設定を読む（ADR 0028）
- 送信の方法、送り先、SMTP の設定は send-to-kindle が扱い、epubize は持たない（ADR 0027）
- Send to Kindle のメールは MOBI を受け付けないため、送るのは EPUB だけとする（ADR 0032）

### 誤り

- 子プロセスが失敗したときは、`<プログラム> が失敗しました（<終了コード>）: <標準エラー出力>` の形で、ボタンの横に出す
- 実行ファイルが見つからないときは、管理画面で指定するよう促す

## 13. 外部のプログラムの探し方と起動

- 実行ファイルは、管理画面の指定、なければ PATH、それでもなければ `~/bin`、`~/.local/bin`、`/opt/homebrew/bin`、`/usr/local/bin` の順に探す（ADR 0018、ADR 0020、ADR 0024）
- クローラーだけは、管理画面の指定がなければ、環境変数 `EPUBIZE_CRAWLER` を PATH より先に見る
- 探す名前は `novel-crawler`、`epub-builder`、`send-to-kindle`、`kindlegen`、`striptool` である
- 子プロセスには、アプリの PATH の後に、上の四つの場所と mise の shim（`~/.local/share/mise/shims`）を足した PATH を渡す。Finder から起動したアプリの PATH には、epub-builder が使う deno がないため（ADR 0029）

## 14. 開発

- 検査とテストは README の「開発」にある。フロントエンドは Biome、vitest、Playwright（Tauri のコマンドとイベントを偽物に差し替えて Vite の開発サーバーに対して動かす）、Rust は rustfmt、clippy、cargo test で行う（ADR 0022）
- CI は `main` への push と pull request で、ADR guard、Frontend、E2E、Tauri（ubuntu-24.04 と macos-15）のジョブを並列に動かす
- `v` で始まるタグを push すると、macOS 向けの `.app` と `.dmg` を作り、下書きの GitHub Release に添付する（署名なし）
- コミット済みの ADR は、`ステータス:` の行を除いて変更できない。Claude Code の hook と git の pre-commit hook で強制する（ADR 0002）
- テストのデータは全て合成したもので、Web 小説の本文などの著作物を含まない

## 15. 扱わないこと

- 取得（クロール）の実装（ADR 0001）
- 外部のプログラムの同梱
- 整形の設定の組方向と removeEmptyLine 以外の項目を、本文に適用すること（ADR 0029、ADR 0030）
- 保存の場所を選ぶダイアログ（ADR 0029）
- 送ったことのない話だけを送る選び方（ADR 0029）
- 表紙（`meta/cover`）を作ること（ADR 0031）
- EPUBCheck を epubize から掛けること（ADR 0029）
- Kindle に MOBI を送ること（ADR 0032）
- `book.toml` に `primary_writing_mode` を明示して書くこと（ADR 0034）
- Windows と Linux 向けの配布物

## 16. ADR の状態

置き換えた ADR と、一部を後の ADR で改めた ADR は次のとおりである。破棄した ADR はない。

| ADR  | 状態                                                                                                        |
| ---- | ----------------------------------------------------------------------------------------------------------- |
| 0005 | 本文の Markdown の規約の一部を ADR 0009 で置き換えた                                                        |
| 0007 | 記録の置き場所を ADR 0014 で改めた                                                                          |
| 0010 | ADR 0012 で置き換えた                                                                                       |
| 0011 | 整形の設定を保存しないことを ADR 0017 で、download mobi のボタンを ADR 0025 で改めた                        |
| 0012 | ADR 0022 で置き換えた                                                                                       |
| 0013 | データベースの置き場所を ADR 0014 で改めた                                                                  |
| 0016 | クローラーの探し方を ADR 0018 で改めた                                                                      |
| 0018 | クローラーの探し方を ADR 0020 で改めた。押せないボタンは ADR 0029 で実装した                               |
| 0019 | 決め打ちの時刻を ADR 0020 で改めた                                                                          |
| 0025 | mobi への対応をやめることを、端末に直接入れるための download mobi について ADR 0032 で改めた                |
| 0029 | 整形の設定の removeEmptyLine を使わないことを ADR 0030 で、`primary-writing-mode` を書かないことを ADR 0034 で改めた |
| 0032 | `primary-writing-mode` を書かないことを ADR 0034 で改めた                                                    |
| 0033 | `primary-writing-mode` を書かないことを ADR 0034 で改めた                                                    |
