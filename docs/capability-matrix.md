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
| 定義・dispatch・visibility | supported | class / module / mixin / constant / method / `super`。`self` 以外の明示 receiver から private method（RBS の `def self?.` で定義された Kernel 関数を含む）は呼べないため `untyped`。別 file の workspace class や名前だけ知られた class は、見えない定義が後で解決できるよう呼び出しを遅延参照として残す。bare な `private` は `def self.x` に影響しない |
| 変数・flow | supported | local / ivar（method 内の再代入・分岐・複合代入） / 多重代入 / narrowing（標準 `Class#===` による postfix の local 代入と、閉じた subject の case。`when` の operand は定数参照に限らず、型が `singleton(C)`（またはその union）なら class として絞り込む） / 分岐間のlocal相関 / rescue。`x = Foo.new` の local が生成 attr 以外に使われない間は、最後の生成 writer の値を reader で読む（形が変わる collection は対象外） |
| block・高階 API | supported | Proc、lambda、yield、Enumerable、Enumerator、Lazy。source method が block を実行しない / 必ず実行する / 実行し得るかを本文の flow から要約し（引数の値には依存しない）、block 内で代入した外側 local を保持 / 置換 / join する（定義順で未解析の callee は join）。Proc / lambda リテラルの呼び出しは method と同じ規則で optional / rest / post / keyword / block 引数を束縛し、proc の単一配列引数の展開、lambda 内の `return` / `break`、定義したメソッド内の lambda 呼び出しによる外側 local への代入も反映する |
| collection shape | supported | Array、Hash、Set、Tuple、Record（静的 symbol key を持つキーワード splat の Record union を含む）。splat と `Kernel#Array` は `to_a` 変換に従い、Tuple は形を保ち、要素数が分からない splat は `Array[T]` にする。local Hash へ書いた key は receiver / 非静的 key の別用途まで存在するものとして読み、`{}` から始まる local は異なる静的 key を不在とする。Hash の union では、key と value の型がどちらも別の Hash メンバーに含まれるメンバーを吸収し、条件付きの `h[k] = v` が続いても union を倍増させない |
| pattern matching | partial | version gate、array / hash / find binding、閉じた Record union では必須のリテラルキーを持つ hash pattern で枝を絞り込み、hash class pattern は標準 `Class#===` のclass predicateを先に評価（source override・未解決class・不明な継承は枝を保守的に維持）、literal / expression subject のcapture型維持、guard 内の capture、rightward assignment の正常経路で subject local を絞り込み、`case/in` の不一致は例外経路。数値リテラル pattern は class をまたぐ数値の等価（`1.0 === 1`）を到達可能とし、guard なしで先に不一致となった値と等しい後続の数値リテラル枝と、非ゼロの虚数リテラルが実数 subject に一致する枝だけを除く。`&&` / `||` / `!` / 条件式 / guard の捕捉変数はその経路で評価された operand だけから束縛し、未確定 guard の else へも引き継ぐ |
| 動的定義 | partial | 静的な名前の `define_method`、yield する source method に渡した class-body block 内の `def`、`attr_*`（静的な Record / Tuple iterator 内の束縛名と、判定可能な `next if` / `next unless` を含む）、`Struct.new`、`Data.define`（位置引数・キーワード引数の `new` 呼び出しからmember型を推定） |
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
| ActiveRecord / ActiveModel | supported | association、scope、enum、attribute、schema、relation / association proxy API |
| ActiveSupport | supported | Concern、delegate、class attribute、core extension（`starts_with?` / `ends_with?`、`Array#second` / `including` / `in_groups`、`Hash#symbolize_keys` / `reverse_merge`、`Duration` 演算、`TimeWithZone` など）、`StringInquirer` の predicate、Rails 有効時の `String#underscore` / `String#camelize` の戻り型推論と missing-method 診断、`try` / `try!` の nil-safe 呼び出し |
| ActionController / ActionMailer / ActiveJob | supported | class-body DSL と主要 instance API。enqueue 系は callback の中断で `false` を返す（Rails 7.0 以降） |
| routes / structure.sql | supported | Rails project fixture を含む。routes は scope / resources / shallow / concern / direct の命名規則を扱い、`draw` による分割ファイルは未対応 |
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
