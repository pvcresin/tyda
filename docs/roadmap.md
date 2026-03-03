# Roadmap

未完了の設計課題だけを置く。完了した項目、実装履歴、計測メモは削除する。
対応範囲は [capability-matrix.md](capability-matrix.md)、設計判断は [design.md](design.md)、
性能基準は [performance.md](performance.md) を参照する。

## 高

### メモリ

- [ ] snapshot container の殻を圧縮する
- [ ] typed dependency referencer を圧縮する
- [ ] LSP incremental state を Symbol / Fragment / SCC 単位へ整理する
- [ ] class data の名前キーと重複保持を整理する
- [ ] `ClassData` の共有と copy-on-write を広げる
- [ ] Batch merge の call site copy を共有化する

### 診断・速度

- [ ] final diagnostics の definition 二重収集を解消する

## 中

- [ ] Concern の includer 依存 DSL を拡張する（schema fallback、Devise、AMS など）
- [ ] stdlib RBS の canonical shape 共有を進める
- [ ] 大規模 Rails での `--diagnostics` context scan を高速化する
- [ ] LSP workspace scan の重複実行を世代管理で防ぐ
- [ ] Ruby / Rails version、framework DSL、scenario coverage を継続する

## 低

- [ ] `.rbs` / `.rbi` 宣言の visibility を取り込む
- [ ] deferred param receiver の連鎖・arity 依存ケースを広げる
- [ ] `argument_type_mismatch` の arity / external param 対応を広げる
- [ ] receiver 無し `configure` block の self を供給する
- [ ] LSP references / completion を拡張する
- [ ] `ActiveRecord.find` の引数 overload を追加する
- [ ] stdlib nested declaration の汎用 index を追加する

## 健全性

- [ ] 一時 debug API を bench 専用へ移すか削除する
- [ ] demand-driven receiver post-pass を再評価する
- [ ] `DirtyPattern`、`CallSiteStore`、LSP scan state を段階的に整理する

個別 repository の runtime DSL 登録（Redmine など）は対象外。型が必要な場合は外部 RBS / RBI
を入力する。
