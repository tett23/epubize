# epubize

Web 小説を EPUB にするツールと、その管理 GUI。

取得(クロール)は別の非公開リポジトリが担い、このリポジトリは取得済みのデータから
EPUB を生成し、作品を管理する部分を受け持つ。GUI は Tauri で動き、データは epubize の SQLite に持つ。

クローラーは子プロセスとして起動し、標準出力の JSON Lines で結果を受け取る。
形式は [schema/crawler-output.v1.schema.json](schema/crawler-output.v1.schema.json) と
[ADR 0005](docs/adr/0005-crawler-data-format.md) にある。クローラーは同梱しない。

設計上の決定とその理由は [docs/adr/](docs/adr/) に記録している。

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
