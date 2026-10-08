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

## ライセンス

[MIT](LICENSE)
