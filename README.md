# epubize

Web 小説を EPUB にするツールと、その管理 GUI。

取得(クロール)は別の非公開リポジトリが担い、このリポジトリは取得済みのデータから
EPUB を生成し、作品を管理する部分を受け持つ。GUI は Tauri で動き、データは epubize の SQLite に持つ。

クローラーは子プロセスとして起動し、標準出力の JSON Lines で結果を受け取る。
形式は [schema/crawler-output.v1.schema.json](schema/crawler-output.v1.schema.json) と
[ADR 0005](docs/adr/0005-crawler-data-format.md) にある。クローラーは同梱しない。

設計上の決定とその理由は [docs/adr/](docs/adr/) に記録している。

## 使う準備

macOS で、作品の取得から EPUB の書き出し、Kindle への送信までを使うための手順。

### 1. epubize を入れる

Node.js、pnpm、Rust を入れてから、release ビルドを作り、`/Applications` に置く。

```bash
pnpm install
pnpm tauri build --bundles app
cp -R src-tauri/target/release/bundle/macos/epubize.app /Applications/
```

GitHub の Release にある `.dmg` を使ってもよい。署名していないため、初回は開けないと言われる。システム設定の「プライバシーとセキュリティ」で、epubize を開くことを許可する。

release ビルドは `production` の環境で動く。データの置き場所は下の「開発」の節にある。

### 2. 外部のプログラムを入れる

epubize は次の三つを子プロセスとして起動する。PATH、`~/bin`、`~/.local/bin`、`/opt/homebrew/bin`、`/usr/local/bin` から自動で探す。
見つからないときや別のものを使うときは、管理画面(settings)で実行ファイルを指定する。

| プログラム       | 使うところ                                       | 入れ方                                                                                                |
| ---------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| `novel-crawler`  | fetch(作品と話の取得)                            | 非公開のため別に用意する                                                                              |
| `epub-builder`   | download epub、send to Kindle(EPUB 3.0 の生成)   | [tett23/epub-builder](https://github.com/tett23/epub-builder) を clone して `deno task install`       |
| `send-to-kindle` | send to Kindle(メールでの送信)                   | [tett23/dotfiles](https://github.com/tett23/dotfiles) の `bin/send-to-kindle` を `~/bin` などに置く(シンボリックリンクでよい) |

epub-builder と send-to-kindle は Deno で動く。Finder から起動したアプリはシェルの PATH を引き継がないため、
epubize は子プロセスの PATH に [mise](https://mise.jdx.dev/) の shim(`~/.local/share/mise/shims`)を足す。
Deno は mise で入れておく(`mise use -g deno`)。

### 3. Kindle への送信を設定する

send-to-kindle は SMTP でメールを送る。このリポジトリの [.env.example](.env.example) を、アプリ用データ領域に `.env` として写し、値を書き込む。
`.env.example` も同じ場所に置くと、`.env` に足りないキーを管理画面に出す([ADR 0028](docs/adr/0028-send-to-kindle-env-files.md))。

```bash
dir="$HOME/Library/Application Support/com.github.tett23.epubize/production"
mkdir -p "$dir"
cp .env.example "$dir/.env.example"
cp .env.example "$dir/.env"
chmod 600 "$dir/.env"
```

- `EMAIL`(送信元のメールアドレス)を、Amazon の「コンテンツと端末の管理」の「承認済み E メールアドレス一覧」に加える
- `SEND_TO_KINDLE_EMAIL` には、同じページの端末の欄にある `@kindle.com` のアドレスを書く
- Gmail で送るときは、`SMTP_HOST=smtp.gmail.com`、`SMTP_PORT=587`、`SMTP_USER_NAME` に Gmail のアドレス、
  `SMTP_PASSWORD` に Google アカウントの「アプリ パスワード」を書く

`.env` の場所は、管理画面で変えられる。

### 4. 作品を購読する

画面の上の欄に作品の URL を入れて add を押すか、`~/.config/epubize/production/novels.json` に書く(kindlize と同じ形)。
fetch all で、購読している作品の目次と本文を取得する。管理画面で、毎日決まった時刻に fetch all を行うように設定できる。

### 5. 確かめる

管理画面(settings)で、三つのプログラムの「使用中」にパスが出ていること、`.env` に足りないキーがないことを確かめる。
作品のページの download epub で `~/Downloads` に EPUB ができれば、epub-builder まで動いている。
話を一つ選んで send to Kindle を押し、Kindle に届けば準備は終わり。

## 開発

clone した後に一度、コミット済み ADR の変更を拒否する git の hook を有効にする(要 Deno)。

```bash
git config core.hooksPath .githooks
```

コミット済みの ADR は、ステータス行以外を変更できない([ADR 0002](docs/adr/0002-immutable-adrs.md))。

GUI は Tauri + React + TypeScript で、パッケージ管理は pnpm を使う([ADR 0006](docs/adr/0006-react-typescript-pnpm-frontend.md))。
Node.js、pnpm、Rust が必要になる。

```bash
pnpm install
pnpm tauri dev
```

検査とテスト([ADR 0022](docs/adr/0022-development-infrastructure.md))。E2E テストは、Tauri のコマンドを偽物に差し替えて Vite の開発サーバーに対して動かす。

```bash
pnpm lint                          # Biome による lint と整形の検査(pnpm format で直す)
pnpm test                          # vitest
pnpm exec playwright install chromium  # 初回だけ
pnpm e2e                           # Playwright
(cd src-tauri && cargo test)
```

配布物は、`v` で始まるタグを push すると GitHub Actions が macOS 向けの `.app` と `.dmg` を作り、下書きの Release に添付する(署名なし)。

データは `development` と `production` の環境ごとに分けて置く([ADR 0014](docs/adr/0014-subscriptions-json-and-environment-directories.md))。
環境は `EPUBIZE_ENV` で指定でき、指定しなければ debug ビルドは `development`、release ビルドは `production` になる。

- 購読している作品: `~/.config/epubize/<環境>/novels.json`(kindlize と同じ形)
- SQLite: `<アプリ用データ領域>/com.github.tett23.epubize/<環境>/epubize.sqlite3`
- ウィンドウの位置と大きさ: `<アプリ用設定領域>/com.github.tett23.epubize/<環境>/window-state.json`

データは SQLite に持ち、起動時に未適用のマイグレーションを自動で適用する([ADR 0013](docs/adr/0013-sqlite-migrations.md))。
マイグレーションは `src-tauri/migrations/` に置き、[mise](https://mise.jdx.dev/) のタスクから操作できる。

```bash
mise run db:new create_novels  # 次の版の空のマイグレーションを作る
mise run db:migrate            # 未適用のマイグレーションを適用する
mise run db:status             # 適用済みの版と未適用のマイグレーションを表示する
mise run db:reset              # データベースを消して全てのマイグレーションを流し直す
```

## ライセンス

[MIT](LICENSE)
