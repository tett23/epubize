# ADR 0022: フロントエンドを Biome で検査し、画面を Playwright で試験し、CI を macOS でも動かし、配布物を作るワークフローを置く

ステータス: 採択

## 文脈

CI は ADR 0012 で決めた。そこでは、フロントエンドの lint と、画面を実際に操作する E2E テストを、仕組みを入れるときに決めるとしていた。
管理画面の購読の remove と、作品の remove episodes は、単体テストでは通っていたが、実際のアプリでは動かなかった（ADR 0023）。画面の操作を自動で試験する仕組みがなかったためである。
主な利用環境は macOS だが、CI は Linux だけで動いていた。配布物（`.app`、`.dmg`）を作る手順もなかった。
Tauri の WebDriver（tauri-driver）は macOS に対応していない。

## 実装すること

- フロントエンドの lint と整形に Biome を使う。設定は `biome.json` に置く
  - 対象は `src/`、`e2e/`、直下の設定ファイルとする。Rust、Deno の hook のスクリプト、ADR、スキーマは対象にしない
  - React の規則（hook の依存の検査を含む）を有効にする。規則を外すときは、`biome-ignore` に理由を書く
  - `pnpm lint` で検査し、`pnpm format` で直す
- 画面の E2E テストに Playwright を使う。テストは `e2e/` に置き、`pnpm e2e` で動かす
  - Vite の開発サーバーに対して、Chromium で動かす
  - Tauri のコマンドとイベントは、ページの読み込みの前に入れる偽物（`e2e/backend.ts`）で差し替える。偽物はページの中に状態を持ち、テストから呼ばれたコマンドを調べ、イベントを流せる
  - データは全て合成したものとする
- CI（`.github/workflows/ci.yml`）は、`main` への push と pull request で次のジョブを並列に動かす
  - ADR guard：`deno test --no-config --no-lock test/`
  - Frontend：`biome ci`、`pnpm build`（型検査と Vite のビルド）、`pnpm test`（vitest）
  - E2E：Playwright の Chromium を入れ、`pnpm e2e`。失敗したときは trace を成果物として残す
  - Tauri：Linux（ubuntu-24.04）と macOS（macos-15）の両方で、`cargo fmt --check`、`cargo clippy --all-targets --locked -- -D warnings`、`cargo test --locked`。macOS では、debug ビルドの `.app` も作る
- CI のワークフローの権限は `contents: read` だけとし、同じブランチで新しい実行が始まったら古い実行は取り消す
- pnpm の版は `package.json` の `packageManager` で固定し、CI と手元で揃える。pnpm はこの版を `pnpm-lock.yaml` にも記録するため、`packageManager` を変えたらロックファイルも更新する
- Rust は stable を使う。Linux の Tauri のジョブでは WebKitGTK などの依存を apt で入れる
- ADR の変更を CI で検査することはしない（ADR 0002）
- 配布物を作るワークフロー（`.github/workflows/release.yml`）を置く
  - `v` で始まるタグの push と、手で動かしたときに、macOS で release ビルドの `.app` と `.dmg` を作り、成果物として残す
  - タグの push のときは、`.dmg` を添付した下書きの GitHub Release を作る
- これにより ADR 0012 を置き換える

## 実装しないこと

- Tauri の WebDriver による、実際のアプリの E2E テストはしない。macOS で動かないため。実際のアプリでの確かめは、変更のたびに手で行う
- アプリの署名と公証はしない。証明書が要るため、配布を広げるときに決める
- Windows 向けの配布物は作らない
- E2E テストで、実際のクローラーやデータベースは使わない

## テスト設計

- E2E テスト（`e2e/app.spec.ts`）で、トップの一覧と未取得、購読の追加とエラー、fetch all と取得の結果、クローラーがないときのボタン、管理画面の保存と反映、正しくない値の拒否、購読を外すときの確認（やめる、実行する）、remove episodes の後の読み直し、話のページの挿絵の差し替えと外部への読み込みがないことを確かめる
- remove episodes の後の読み直しを外すと E2E テストが失敗することを確かめた
- 手元で release ビルドの `.app` を作って起動し、`production` のデータベースを開き、見出しに `production` と出て、購読とクローラーと定期取得の表示が正しいことを確かめた
- CI と配布のワークフローは、push した後に GitHub 上で通ることを確かめる

## トレードオフ

- E2E テストは Tauri のコマンドを偽物に差し替えるため、Rust の側との食い違いは見つけられない。Rust の側は単体テストで、両者のつながりは実際のアプリで確かめる
- macOS の CI は Linux より遅く、使える時間も多く消費する
- 署名しない `.app` は、macOS の Gatekeeper で開くときに確認が要る
