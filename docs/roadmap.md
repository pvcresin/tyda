# Roadmap

未完了の設計課題だけを置く。完了した項目、実装履歴、計測メモは削除する。
対応範囲は [capability-matrix.md](capability-matrix.md)、設計判断は [design.md](design.md)、
性能基準は [performance.md](performance.md) を参照する。

## 高

### メモリ

- [ ] snapshot container の殻を圧縮する
- [ ] LSP incremental state を Symbol / Fragment / SCC 単位へ整理する
- [ ] `ClassData` の共有と copy-on-write を広げる
- [ ] Batch merge の call site copy を共有化する

### 診断・速度

- [ ] coverage を file / source-site 単位で base/head 比較し、aggregate では隠れる局所的な `typed` → `untyped` / `unknown` 化をCIで検出する

### AI agent 支援

agent が自ら使うのは、grep・ファイル全体の読み込み・テストによる誤り探しを、少ない token と手順で置き換える機能だけである。入口は少数に絞り、編集後の hook など context に直接返す経路を優先する。誤検出を出さない現在の診断方針は維持する。

- [ ] 常駐 server を用意し、CLI・MCP・hook が LSP と同じ `WorkspaceState` に接続する。毎回の全体解析をやめ、編集ごとの応答をミリ秒単位にする
- [ ] MCP を workspace 対応にする。現在は渡された source と同梱 stdlib RBS だけを解析するため、実プロジェクトでは agent が snippet を組み立てる必要がある
- [ ] MCP の位置指定に、行・UTF-16 列に加えてシンボル名（`User#full_name`）を受け付ける
- [ ] 推論シグネチャを RBS 形式で返すファイル / クラス outline を MCP と CLI に追加する
- [ ] レシーバ型で絞った references / callers を LSP と MCP に追加する。名前一致の候補とは「確実 / 可能性あり」で区別する
- [ ] 変更ファイルと影響範囲だけを検査し、前回から増えた診断だけを短いテキストと exit code で返す check を追加する。agent 向けの hook テンプレートも用意する
- [ ] `explain_type_at_position` を workspace 全体に広げ、`untyped` がどこから流れ込んだかを示す
- [ ] シグネチャ変更で影響を受ける呼び出し箇所を返す
- [ ] クエリごとに時間の予算を設け、超過した部分は `untyped` と diagnostic を返す（固定の深さ制限は使わない）
- [ ] MCP 設定・hook・agent 向け指示をまとめて生成する導入コマンドを追加する
- [ ] subject 上の修正タスクで、tyda の有無による token 数・tool 呼び出し回数・成功率を比べる評価を用意し、tool の追加や説明文の改善はこの結果で判断する

### Tyda 開発の高速化

- [ ] 型が決まるまでの規則・deferred ref・SCC の経路を出力する `trace` を追加する（上記の explain と同じ基盤）
- [ ] subject 差分の後退から最小シナリオを自動で切り出す
- [ ] `inference.rs`・`registry.rs`・`lsp.rs` を責務ごとに分割し、agent の読み込みとビルドの負荷を下げる

## 中

- [ ] Concern の includer 依存 DSL を拡張する（schema fallback、Devise、AMS など）
- [ ] 大規模 Rails での `--diagnostics` context scan を高速化する
- [ ] block callback の副作用解析を一般化し、外側で nil check 済みのivarがcallback内で誤って `nil` と診断されるケースを見直す（Redmine `Attachment#files_to_final_location` の `sha.update(@temp_file)`）。実際のmutationは引き続きnarrowingを無効化し、クラス・メソッド固有の例外は追加しない
- [ ] CLI のファイル単位推論で、後のファイルに依存する型を遅延参照として保ち、全体の集約後に解決する。別ファイルの model への `find(1)` が `untyped` になる、後のファイルで定義されるクラスの `when` が引数を `untyped` にする、の2件（`tests/cli.rs` の `when_with_class_from_later_file_does_not_leak_other_arguments`）。シナリオ runner は他ファイルを先に解析するため、CLI でのみ再現する
- [ ] CLI と scenario runner の結果をそろえる。単一ファイルでも、呼び出し側から型が決まる parameter への演算（`def foo(x) = x + 1` を `foo(1)` で呼ぶ）、`*args` の要素、class 内の再帰が CLI だけ `untyped` になり、別ファイルの定数参照もディレクトリ指定の CLI で `untyped` になる。scenario は CLI 経路を確かめないため、両経路を比べる仕組みも用意する
- [ ] Ruby の意味論どおり、未初期化の ivar の値に `nil` を含める。`initialize` のすべての経路（class instance variable は class body）で代入される ivar だけを非 nil とし、それ以外の reader と参照は `T?` にする。この `nil` による `missing_method` は除外せずに報告する。意図して使う箇所は `# tyda: ignore[missing_method]` で抑制する。既存 scenario の期待値も合わせて直す（`known-issues/ruby_methods.md`、`ruby/variable/instance.md` の known issue）
- [ ] TODO: 未初期化の ivar に由来する `nil` 診断の重さを検討する（他の `nil` と severity を分けて warning にするか、など）
- [ ] 保存前の ActiveRecord model を推論で区別する。既定は tapioca・rbs_rails と同じく保存済みで valid な model の契約とし、`null: false` のカラムと必須の `belongs_to` は非 nil にする。そのうえで `find` / `where` / association から来た値は非 nil のまま、`new` / `build` から来てまだ保存していない値だけカラムと association を `nil` を含む型にする。値の出どころを追えない RBI 生成ツールは全体を nilable にする設定（tapioca の `ActiveRecordColumnTypes`）しか持てないが、Tyda は推論で実際に危ない箇所だけを捉える。全体を切り替える設定は、要望が出るまで追加しない
- [ ] `untyped` を含む union を扱う規則をそろえる。呼び出し側の引数が `Foo | untyped` だとパラメータが `Foo` になり、callee で missing method の誤検出が出る（`known-issues/call_unions.md`）
- [ ] 制御フロー上の値を保つ。自己再帰する述語の戻り値が `bool` に `untyped` を含む（`known-issues/flow_values.md`）。また、単一ファイルの CLI では `File` を渡す呼び出しがあっても `break unless part` の後の `yield part` が `String` にならず、parameter 経由の `IO#read(len)` が `String?` でなく `String` になる。CLI は parameter レシーバへの `file.read(len)` を引数なしの呼び出しとして集約後に解決し、最初の overload（`-> String`）を選ぶ。全体を解析する scenario runner と LSP では正しく推論する（`tests/cli.rs` の `known_issue_io_read_through_parameters_in_a_single_file`）
- [ ] nil の誤検出を減らす。メソッド内で代入した ivar が self 呼び出しの後に nil になる、ivar の truthy ガードが入れ子の修飾子 `if` に届かない、reader の nil ガードが同じメソッドの block に届かない、`*xs.partition` の多重代入が nil になる、ivar の nil ガードがそれを書かない self 呼び出しの後で消える、の5件（`tests/flow_diagnostics.rs` の `known_issue_nil_false_positives_still_reported`）
- [ ] PR 154 の subject 差分で見つけたが、最小再現ができていない後退を調べる。Redmine `link_to_project` の引数が `(Project | untyped)?` から `untyped?` になる。`respond_to?` を確かめる述語メソッドや `before_action` など、別メソッドのガードを越えた nil・missing method の誤検出
- [ ] Ruby / Rails version、framework DSL、scenario coverage を継続する

## 低

- [ ] `.rbi` 宣言の visibility を取り込む
- [ ] deferred param receiver の連鎖・arity 依存ケースを広げる
- [ ] `argument_type_mismatch` の arity / external param 対応を広げる
- [ ] LSP references / completion を拡張する
- [ ] stdlib nested declaration の汎用 index を追加する

## 健全性

- [ ] demand-driven receiver post-pass を再評価する
- [ ] method-return-ref の依存グラフを SCC 単位で解き、収束しない循環を `untyped` にする方式を再考する
- [ ] `DirtyPattern`、`CallSiteStore`、LSP scan state を段階的に整理する

個別 repository の runtime DSL 登録（Redmine など）は対象外。型が必要な場合は外部 RBS / RBI
を入力する。
