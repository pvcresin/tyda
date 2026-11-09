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

## 中

- [ ] Concern の includer 依存 DSL を拡張する（schema fallback、Devise、AMS など）
- [ ] 大規模 Rails での `--diagnostics` context scan を高速化する
- [ ] block callback の副作用解析を一般化し、外側で nil check 済みのivarがcallback内で誤って `nil` と診断されるケースを見直す（Redmine `Attachment#files_to_final_location` の `sha.update(@temp_file)`）。実際のmutationは引き続きnarrowingを無効化し、クラス・メソッド固有の例外は追加しない
- [ ] CLI のファイル単位推論で、後のファイルに依存する型を遅延参照として保ち、全体の集約後に解決する。別ファイルの model への `find(1)` が `untyped` になる、後のファイルで定義されるクラスの `when` が引数を `untyped` にする、の2件（`tests/cli.rs` の `when_with_class_from_later_file_does_not_leak_other_arguments`）。シナリオ runner は他ファイルを先に解析するため、CLI でのみ再現する
- [ ] 別ファイルで定義したクラスの singleton method（`LoadSpec.load` など）を、CLI が private な `Kernel#load` に解決して `bool` を返す。シナリオ runner は他ファイルを先に解析するため、CLI でのみ再現する（`tests/cli.rs` の `known_issue_singleton_method_in_another_file_resolves_to_kernel`）
- [ ] `untyped` を含む union を扱う規則をそろえる。呼び出し側の引数が `Foo | untyped` だとパラメータが `Foo` になり、callee で missing method の誤検出が出る。nil かもしれない `untyped` レシーバへの呼び出し（`(x ? x.y : nil).map`）が `untyped` でなく `Array[untyped]` になる（`known-issues/call_unions.md`）
- [ ] 制御フロー上の値を保つ。自己再帰する述語の戻り値が `bool` に `untyped` を含む（`known-issues/flow_values.md`）。また、単一ファイルの CLI では `File` を渡す呼び出しがあっても `break unless part` の後の `yield part` が `String` にならず、parameter 経由の `IO#read(len)` が `String?` でなく `String` になる。CLI は parameter レシーバへの `file.read(len)` を引数なしの呼び出しとして集約後に解決し、最初の overload（`-> String`）を選ぶ。全体を解析する scenario runner と LSP では正しく推論する（`tests/cli.rs` の `known_issue_io_read_through_parameters_in_a_single_file`）
- [ ] nil の誤検出を減らす。メソッド内で代入した ivar が self 呼び出しの後に nil になる、ivar の truthy ガードが入れ子の修飾子 `if` に届かない、reader の nil ガードが同じメソッドの block に届かない、`*xs.partition` の多重代入が nil になる、ivar の nil ガードがそれを書かない self 呼び出しの後で消える、の5件（`tests/flow_diagnostics.rs` の `known_issue_nil_false_positives_still_reported`）
- [ ] PR 154 の subject 差分で見つけたが、最小再現ができていない後退を調べる。Redmine `link_to_project` の引数が `(Project | untyped)?` から `untyped?` になる。`respond_to?` を確かめる述語メソッドや `before_action` など、別メソッドのガードを越えた nil・missing method の誤検出
- [ ] Ruby / Rails version、framework DSL、scenario coverage を継続する

## 低

- [ ] `.rbs` / `.rbi` 宣言の visibility を取り込む
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
