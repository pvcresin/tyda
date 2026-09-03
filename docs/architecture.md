# Architecture

この文書はコードベースの責務分離と、解析結果が CLI / LSP / playground / test を通る経路を示す。
個別の最適化履歴や一時的な調査結果は記録しない。

## 解析の流れ

~~~text
Ruby / RBS / RBI / project files
        ↓
Prism parse + annotation / project facts
        ↓
FileAnalysisSnapshot / FileFacts
        ↓
WorkspaceState + typed dependency graph
        ↓
registry / inference / query
        ↓
RBS render / diagnostics / LSP display
~~~

CLI、LSP、playground、scenario test は入力の与え方と解決 profile が違うだけで、意味論の中心は
`WorkspaceState` と query backend を共有する。単一ファイルを完全解決して表示する入口は
`analysis::analyze_source_for_display` に集約し、LSP、playground、詳細 CLI、通常の Ruby scenario
は同じ snapshot / registry / query 経路を使う。

## 主要コンポーネント

| 層・ファイル | 責務 |
| --- | --- |
| `analysis.rs` / `parser.rs` | Prism 解析、annotation 抽出、共通 snapshot 作成、表示整形 |
| `inference/` | Ruby の式・method・receiver・block・flow・DSL の推論 |
| `inference/plugins/` | gem / framework ごとの DSL 展開。`Plugin` / `PluginManifest` を 1 plugin 1 file で登録 |
| `registry.rs` / `registry/` | class、method、constant、ivar、mixin、call site と型宣言の索引 |
| `types.rs` | `Type` と未解決参照の表現、union / generic / complexity の制御 |
| `query.rs` | snapshot と registry に対する Hover、completion、definition などの query |
| `workspace_state.rs` | snapshot、export fingerprint、dirty state、workspace registry の管理 |
| `dep_graph.rs` | superclass / mixin / method call など種別付き依存と差分伝播 |
| `rbs/` | inline RBS、`.rbs`、stdlib の import / lazy load / render |
| `sorbet/` | `sig`、`.rbi`、Sorbet comment の実験的な import |
| `rails/` | project 検出、schema / routes / inflector、Rails 共通情報 |
| `diagnostics.rs` | missing method、unresolved constant、argument mismatch などの判定 |
| `lsp.rs` | LSP protocol、document / file cache、CodeLens、Hover、refresh |
| `main.rs` | CLI 入力展開、batch projection、RBS / diagnostics の出力 |

Sorbet `sig { ... }` のブロック本体は型 DSL として扱い、通常コードとして推論しない。ブロック内の
`returns` / `void` / `params` を enclosing class のメソッド呼び出しとして診断しない（実行時の
self は `T::Private::Methods::DeclBuilder` であるため）。

## Semantic backend

### `WorkspaceState`

ファイルごとの `FileAnalysisSnapshot`、export fingerprint、依存 edge、workspace registry を
保持する。export が変わらない body-only edit は依存先を再計算せず、変更した symbol から
reverse edge へ dirty を伝播する。

解決 profile は次の二つだけに分ける。

- `Batch`: CLI の一括解析。targets と context をまとめて projection し、決定的に render する。
- `Interactive`: LSP の長寿命 workspace。open file の最新 snapshot と dirty state を使い、必要な
  query だけを再評価する。

scenario test は小さな workspace を case ごとに作り、同じ backend の projection を検証する。
表示用の hover / definition 索引は意味解析後の一時 registry で収集し、遅延ロードした外部型や
探索用の事実が意味解析結果へ混ざらないようにする。入口ごとに別の型解決を追加しない。

### 知識源の合成

Ruby source、RBS、RBI、schema、plugin の知識は同じ registry に合成する。優先順位は
「実体のある source / 宣言を保ち、未解決の空 stub で権威ある定義を隠さない」ことを基本とする。
合成規則を変更したときは、該当する registry unit test と scenario を同じ変更で更新する。

framework DSL は library-scoped に有効化し、plugin のフックは `PluginCx` を通す。個別 repository の
runtime DSL 登録を解析コアへ持ち込まず、動的な API は外部 RBS / RBI を入力する。

### 外部型

stdlib RBS と project の RBS / RBI は必要な file / class を lazy に読み込む。parse 結果は
共有可能な形に変換し、file 固有の推論状態と混ぜない。外部定義の変更は LSP の reload で
関連 cache を無効化する。

## CLI batch

通常の RBS 出力は指定された Ruby ファイルを解析する。`--diagnostics` では診断対象と
workspace context を分ける。

1. **targets**: CLI 引数から展開したファイル。診断と詳細 snapshot を保持する。
2. **context**: targets 外の Ruby ファイル。定義・superclass・mixin・定数などの骨格だけを読む。
3. チャンク単位で並列解析し、`workspace_state::BatchProjectionBuilder` でチャンクが出来上がるたび
   即時に registry へ merge する（全ファイルの snapshot を同時に保持しない）。順序はチャンク順、
   チャンク内はファイル順を保つ。`Batch` resolution は全ファイル投入後に一度だけ行う。
4. RBS 出力ではソースもチャンク単位で読み、merge 後に破棄する。preload の並列読み込みは
   DSL 検出だけを行い本文を残さない。`--diagnostics` は message 生成が target の本文を再度
   使うため、targets のソースだけ保持する。
5. diagnostics は targets にだけ出力する。

この分離で、診断のために workspace 全体の詳細 body / call site を常駐させない。

## LSP

`lsp.rs` は protocol と表示を担当し、意味論は `query.rs` / `WorkspaceState` を使う。

- `initialize`、TCP 起動、full / incremental `didChange`、watched file 更新を受け付ける。
- Hover、CodeLens、definition、typeDefinition、completion、diagnostics は共通の query 経路を使う。
- open document は `didOpen` / `didChange` で snapshot を更新し、request 時に全ファイルを読み直さない。
- 初回 workspace scan が終わるまで、未確定の missing method / constant を早期に報告しない。
- refresh は変更をまとめ、保存されていない入力でも CodeLens / Hover が最新 snapshot を見る。
- `didChange` の diagnostics 発行は debounce する（CodeLens refresh の 75ms と同じ形。document
  ごとの generation を持ち、新しい change が来た時点で pending を捨てる）。`didOpen` と scan 完了
  後の再発行は即時。facts 更新（`start_document_cache_update_if_needed`）は非同期で即時に走る。

TypeProf VSCode 拡張との互換性は protocol boundary に限る。推論結果の parity 表は維持しない。

## 有界性と拡張点

- 型の深さ、union の要素数、式木の fuel、collection shape の成長に上限を設ける。
- 上限に達した場合は panic / hang / OOM を避けて `untyped` へ縮退する。
- 名前だけから戻り値を補う恒久テーブルは少数箇所に集約し、追加前に RBS / plugin で表現できないか確認する。
- framework DSL は中央の巨大な if-chain ではなく plugin registry に追加する。
- 個別リポジトリの動的 DSL 登録を解析コアへ持ち込まない。

性能の基準値は [performance.md](performance.md)、未完了の設計作業は
[roadmap.md](roadmap.md) に置く。
