# Capability Matrix

対応状況はカテゴリ単位で管理する。個別のメソッドや gem の完全な列挙はせず、
実例は [tests/scenarios/](https://github.com/pvcresin/tyda/tree/main/tests/scenarios) と plugin の実装を参照する。

- **supported**: 現在の主要経路で回帰テストがある
- **partial**: 静的に追える範囲だけ対応する
- **experimental**: 仕様・網羅性が未確定
- **planned**: 未着手

## Ruby

| 分野 | 状態 | 代表的な範囲 |
| --- | --- | --- |
| 定義・dispatch・visibility | supported | class / module / mixin / constant / method / `super` |
| 変数・flow | supported | local / ivar（method 内の再代入・分岐・複合代入） / 多重代入 / narrowing（標準 `Class#===` による postfix の local 代入と、閉じた subject の case） / 分岐間のlocal相関 / rescue |
| block・高階 API | supported | Proc、lambda、yield、Enumerable、Enumerator、Lazy |
| collection shape | supported | Array、Hash、Set、Tuple、Record |
| pattern matching | partial | version gate、array / hash / find binding、literal / expression subject のcapture型維持、guard 内の capture、`case/in` の不一致は例外経路 |
| 動的定義 | partial | 静的な名前の `define_method`、`attr_*`、`Struct.new`、`Data.define`（位置引数・キーワード引数の `new` 呼び出しからmember型を推定） |
| runtime-only meta programming | partial | 未知の名前・object identity は `untyped` |
| 深さ・union・shape の制限 | supported | 上限超過時は安全に `untyped` |

`case/when` の到達不能枝除外は、Full inference の再解析で標準 `Class#===`、閉じた subject、
完全な祖先情報を確認できた場合に限る。匿名の `Struct.new` / `Data.define` superclass は祖先情報を
不完全として扱う。compact CLI は既存の初回解析を保ち、再解析対象外の
method も初回解析時の推論を保つ。そのため、後置 `Module#===` override がある noarg method では
既知の推論誤差が残る。

主な scenario は [tests/scenarios/ruby/](https://github.com/pvcresin/tyda/tree/main/tests/scenarios/ruby) にある。

## 型情報と診断

| 分野 | 状態 | 代表的な範囲 |
| --- | --- | --- |
| Ruby source inference | supported | CLI / LSP / scenario の主経路 |
| `.rbs` / stdlib RBS | supported | lazy load、generic、overload、interface |
| inline RBS | supported | `#:`、`# @rbs`、block signature、type alias |
| Sorbet `sig` / `.rbi` | experimental | `T::Struct`、`T::Enum`、assertion、lazy merge |
| 型診断 | supported | 確実な argument / return mismatch、既知の missing method / constant |
| experimental diagnostics | experimental | arity、union member missing method |

RBS scenario は [tests/scenarios/ruby/rbs_input/](https://github.com/pvcresin/tyda/tree/main/tests/scenarios/ruby/rbs_input)、
inline RBS は [tests/scenarios/ruby/rbs_comment/](https://github.com/pvcresin/tyda/tree/main/tests/scenarios/ruby/rbs_comment)、
Sorbet は [tests/scenarios/sorbet/](https://github.com/pvcresin/tyda/tree/main/tests/scenarios/sorbet) に置く。

## DSL / framework

| 分野 | 状態 | 備考 |
| --- | --- | --- |
| ActiveRecord / ActiveModel | supported | association、scope、enum、attribute、schema |
| ActiveSupport | supported | Concern、delegate、class attribute、core extension、`try` / `try!` の nil-safe 呼び出し |
| ActionController / ActionMailer / ActiveJob | supported | class-body DSL と主要 instance API |
| routes / structure.sql | supported | Rails project fixture を含む |
| RSpec structural DSL | supported | `let`、`subject`、`described_class` の lexical scope |
| Devise / Doorkeeper / policy | supported | plugin による scope / policy |
| Grape / GraphQL / AMS | partial | plugin ごとに静的に追える範囲 |
| その他の gem | partial | plugin がある API のみ |
| GitLab in-tree | supported | Presenter、CurrentSettings、Metrics、EE 拡張 |
| Redmine の動的登録 | partial | runtime DSL は推測せず、外部定義を利用 |

Rails scenario は [tests/scenarios/rails/](https://github.com/pvcresin/tyda/tree/main/tests/scenarios/rails) に置く。対応 gem の
有効化と実装は `src/inference/plugins/` を正とする。

## Ruby / Rails version

| 分野 | 状態 |
| --- | --- |
| Ruby version detection / syntax gate | supported |
| Rails version detection / relation gate | supported（6.1〜8.x の主要経路） |
