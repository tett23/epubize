# ADR 0008: スタイルは Tailwind CSS で書く

ステータス: 採択

## 文脈

GUI のフロントエンドは React と TypeScript で書く（ADR 0006）。
ADR 0006 では、UI のライブラリは必要になったときに決めるとした。
画面を作り始める前に、スタイルの書き方を決めておく。

## 実装すること

- スタイルは Tailwind CSS（v4）のユーティリティクラスで書く
- Vite のプラグイン `@tailwindcss/vite` で組み込む。PostCSS の設定ファイルや `tailwind.config.js` は置かない
- 全体の CSS は `src/index.css` に置き、`@import "tailwindcss";` で読み込む。テーマの変更が要るときは、同じファイルの `@theme` で行う
- テンプレート由来の `src/App.css` は削除する

## 実装しないこと

- CSS Modules や CSS-in-JS は併用しない
- この時点では、Tailwind の上に作られたコンポーネントライブラリは入れない

## テスト設計

- 導入であり、検証すべき振る舞いを持たないため、テストはない
- `pnpm build` の出力した CSS に、`App.tsx` で使ったクラスが含まれることを確認した

## トレードオフ

- マークアップにクラスが多く並び、読みにくくなる。その代わり、スタイルが要素の側にあり、使われない CSS が残らない
- Tailwind のクラス名と書き方を覚える必要がある
