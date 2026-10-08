# ADR 0012: GitHub Actions の CI で、フロントエンドの単体テストも行う

ステータス: 置換（ADR 0022）

## 文脈

ADR 0010 で GitHub Actions の CI を決めた。
そのときはフロントエンドのテストの仕組みがなかったため、フロントエンドのテストは CI で行わないとした。
ADR 0011 で vitest を入れ、`pnpm test` でフロントエンドの単体テストを動かせるようになった。
前提が変わったため、CI の決定を改めて書く。この ADR は ADR 0010 を置き換える。

## 実装すること

- `.github/workflows/ci.yml` に GitHub Actions のワークフローを置き、`main` への push と pull request で動かす
- ジョブは次の三つとし、並列に動かす
  - ADR guard：`deno test --no-config --no-lock test/` で、`scripts/adr-guard.ts` のテストを行う
  - Frontend：`pnpm install --frozen-lockfile` の後、`pnpm build` で型検査と Vite のビルドを行い、`pnpm test` で vitest の単体テストを行う
  - Tauri：`cargo fmt --check`、`cargo clippy --all-targets --locked -- -D warnings`、`cargo test --locked` を行う。`tauri::generate_context!` がビルド済みのフロントエンドを埋め込むため、先に `pnpm build` を行う
- 実行環境は `ubuntu-24.04` とし、Tauri のジョブでは WebKitGTK などの依存を apt で入れる
- Rust は stable を使う。pnpm の版は `package.json` の `packageManager` で固定し、CI と手元で揃える。pnpm はこの版を `pnpm-lock.yaml` にも記録するため、`packageManager` を変えたらロックファイルも更新する
- 同じブランチで新しい実行が始まったら、古い実行は取り消す
- ワークフローの権限は `contents: read` だけとする

## 実装しないこと

- ADR の変更を CI で検査することはしない。ADR 0002 のとおり、サーバ側での検査は行わない
- macOS や Windows での実行と、配布物の作成（`tauri build`）とリリースはしない。必要になったときに決める
- フロントエンドの lint と、画面を実際に操作する E2E テストは行わない。仕組みを入れるときに決める

## テスト設計

- CI の設定であり、それ自体のテストはない
- 各ジョブと同じコマンドを手元で実行し、通ることを確認した
- push した後に、GitHub 上で各ジョブが通ることを確認した

## トレードオフ

- 主な利用環境は macOS だが、CI は Linux で動かす。macOS に固有の不具合は CI では見つからない。その代わり、実行が速く、依存の導入も簡単である
- Tauri のジョブは依存の導入と Rust のビルドに時間がかかる。`Swatinem/rust-cache` でビルドの結果を再利用して短くする
- Frontend のジョブは、単体テストの分だけ長くなる。jsdom を使うテストがあるため、その起動の時間もかかる
- Actions の版をメジャー版のタグで指定するため、同じメジャー版の中の更新は自動で取り込まれる
