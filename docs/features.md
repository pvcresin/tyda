# Features

Tyda は Ruby / Rails のコードから型を推論し、RBS を出力する CLI と LSP を提供する。
対応状況の正本は [capability-matrix.md](capability-matrix.md)、設計上の線引きは
[design.md](design.md) に置く。

## プロダクト

- Ruby source と `.rbs` を中心にした query 型推論 engine
- CLI の RBS 出力、JSON Lines diagnostics
- TypeProf VSCode 拡張と接続できる LSP server
- Rails / gem DSL plugin
- Sorbet `sig` / `.rbi` の実験的な補助
- wasm playground（[playground/](../playground/)）

## Ruby 推論

主な対応範囲は次のとおり。

- class / module / method / mixin / constant / local / instance variable（LSP / Playground の定義・参照 hover を含む。宣言済み引数の定義位置も対象）
- method dispatch、visibility、`super`、singleton method、refinement（`include` / `prepend` /
  `extend` の適用順を反映。静的に完全な `Module#ancestors` は順序付き `Tuple`）
- block / Proc / lambda / `yield` / Enumerable / Enumerator / Lazy（tuple の要素 union とリテラル演算を block 内へ伝播）
- Array / Hash / Set / Tuple / Record の要素型と shape
- if / case / rescue / safe navigation / pattern matching の flow narrowing
- 多重代入、operator-write、文字列・シンボル補間（静的な literal / union の展開を含む）
- Thread / Fiber / Queue の bounded な値伝播
- 静的に名前を求められる `alias` / `define_method` / `attr_*` / `Struct.new` /
  `Data.define` / `Forwardable` / `send` / `const_get`

実行時にしか決まらない名前、object identity、method surface は推測せず `untyped` にする。
型の深さ・union・collection shape には上限がある。

## 型情報ソース

### RBS

- `.rbs` の自動発見、stdlib の lazy load、generic、overload、interface、alias
- inline RBS（`#:` / `# @rbs`）の単一・複数行宣言、block signature、type alias
- `self` / `instance` を call site の受け手へ解決する factory 型

### Sorbet（実験的）

`sorbet/config` がある project で、`sig`、`T.let`、`T::Struct`、`T::Enum`、`.rbi` と
対応する comment extension を読み込む。RBS と同じ安定性や網羅性は保証しない。

## Diagnostics

型推論が主であり、diagnostics は補助である。権威ある宣言に対する確実な不一致だけを報告する。

| 種類 | 既定の severity | 方針 |
| --- | --- | --- |
| `argument_type_mismatch` | error | 宣言 param と actual が確実に不一致のときだけ |
| `missing_method` | warning | receiver と祖先の method surface が完全に既知のときだけ |
| `unresolved_constant` | information | receiver 文脈で未定義と証明できるときだけ |
| `arity_mismatch` | experimental | `TYDA_EXPERIMENTAL_CHECKS=1` のときだけ |

Unknown、`untyped`、開いた `method_missing` 面、未解決の祖先は誤検知を避けて沈黙する。
詳細は [incomplete-code-policy.md](incomplete-code-policy.md) を参照する。

診断を一行だけ抑制するには、対象式の行末に `# tyda: ignore` を置く。特定の種類だけを
抑制する場合は `# tyda: ignore[missing_method]` のように診断 code を指定できる。現在の
code は `missing_method`、`argument_type_mismatch`、`unresolved_constant` などで、CLI・LSP・
Playground で同じ書式を使う。コメントは同じ行の末尾に置いた場合だけ有効で、単独行の
コメントが次の行へ影響することはない。対応する診断がない ignore は `unused_ignore` warning
になり、診断が解消したあとに不要な抑制を見つけられる。

`--diagnostics` の JSON Lines 出力は実行間で byte-identical になる。ファイルは
辞書順の走査順（CLI に明示的に渡したパスはその順序を保つ）、ファイル内の各行は
position 順（line/column 昇順）で並ぶため、複数回実行した出力をそのまま diff できる。

## Rails / gem DSL

Rails の ActiveRecord、ActiveModel、ActiveSupport、ActionController、ActionMailer、ActiveJob、
routes、schema と、主要な gem plugin を扱う。対応 gem の正本は `src/inference/plugins/` であり、
個別 gem の一覧をこの文書に重複して持たない。

- schema / `attribute` / relation から model の型を補う
- Concern、association、scope、enum、delegate などの静的な DSL を展開する
- Grape、GraphQL-Ruby、Devise、Doorkeeper、Sidekiq、Draper、AASM などを plugin で補う
- GitLab in-tree の Presenter / CurrentSettings / Metrics / EE 拡張を検出する
- Redmine の `acts_as_*` など、ライブラリ内部の実行時登録は展開しない。必要なら手書き
  `.rbs` / `.rbi`、または生成済み定義を入力する

## LSP / Playground

LSP は TypeProf VSCode 拡張が期待する起動・version・request 契約に対応する。

- incremental text sync（full change も受理）
- Hover、CodeLens、definition、typeDefinition、completion
- diagnostics の publish と workspace refresh
- CLI と共通の `WorkspaceState` / query backend

Playground は同じ LSP 表示経路を wasm で実行し、Ruby と手書き RBS の結果をブラウザで確認する。

## 主なコマンド

~~~bash
cargo run -- <path>                         # RBS
cargo run -- --diagnostics <path>           # JSON Lines diagnostics
cargo run -- --lsp                          # LSP server
cargo run -- --include-synthetic-dsl-methods <path>
mise run dev                                # playground
~~~

開発・拡張の手順は [development.md](development.md)、テストの追加方法は
[testing.md](testing.md) を参照する。
