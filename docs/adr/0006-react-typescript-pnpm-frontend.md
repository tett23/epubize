# ADR 0006: Tauri のフロントエンドを React と TypeScript で書き、パッケージ管理に pnpm を使う

ステータス: 採択

## 文脈

GUI は Tauri で作る（ADR 0003）。
Tauri はフロントエンドに任意の Web 技術を使えるため、フレームワークとパッケージマネージャを決める必要がある。
ADR 0002 で、hook のために Deno を開発環境に入れたが、アプリケーションの実行環境は別に決めるとした。

## 実装すること

- `create-tauri-app` の `react-ts` テンプレートを元に、リポジトリの直下に置く
  - フロントエンド（Vite、React、TypeScript）は `src/`、`index.html`、`vite.config.ts` に置く
  - Rust 側は `src-tauri/` に置く
- パッケージマネージャは pnpm とし、`pnpm-lock.yaml` をコミットする
- 開発は `pnpm tauri dev`、配布物の作成は `pnpm tauri build` で行う
- テンプレートのデモ（`greet` コマンド、ロゴ、`tauri-plugin-opener`）は取り除き、空の画面だけを持つ状態から始める
- アプリの識別子は `com.github.tett23.epubize` とする

## 実装しないこと

- フロントエンドのコードを Deno で動かすことはしない。Deno は ADR 0002 の hook のためだけに使う
- この時点では、状態管理やルーティング、UI のライブラリは入れない。必要になったときに決める
- モバイル（iOS、Android）向けの初期化はしない

## テスト設計

- 雛形の導入であり、検証すべき振る舞いを持たないため、テストはない
- `pnpm build`、`cargo build`、`cargo clippy` が通り、`pnpm tauri dev` でウィンドウが起動することを確認した
- フロントエンドのテストの仕組みは、テストすべきコードを書くときに決める

## トレードオフ

- 開発環境に Node.js、pnpm、Rust、Deno が必要になる
- React は他の候補より実行時のサイズが大きい。その代わり、周辺のライブラリと事例が最も多い
- テンプレートに沿うため、Tauri の更新に追従しやすい一方で、テンプレート由来の設定（Vite のポート 1420 など）を引き継ぐ
