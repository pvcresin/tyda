# Features

Tyda は Ruby / Rails のコードから型を推論し、RBS を出力する CLI、LSP、AI agent 向け MCP server を提供する。
対応状況の正本は [capability-matrix.md](capability-matrix.md)、設計上の線引きは
[design.md](design.md) に置く。

## プロダクト

- Ruby source と `.rbs` を中心にした query 型推論 engine
- CLI の RBS 出力、JSON Lines diagnostics、型カバレッジ JSON
  - RBS 出力は公式 parser が受理する構文に限る。戻り値の union / intersection は括弧で囲み、
    型注釈のない `&block` は `?{ (?) -> untyped }`、Float literal は `Float` で出す。`void` を含む
    union は使える値がないため `void` にまとめる
  - class / module を定数参照で代入した定数（`A = B`、alias の連鎖、RBS の `class A = B` /
    `module A = B` 入力）は、定数宣言ではなく対象の種類に合わせた `class A = B` / `module A = B` で出す。
    `Class.new` の結果のように型だけが `singleton(...)` の値定数は従来どおり定数宣言にする
- TypeProf VSCode 拡張と接続できる LSP server
- stdio MCP server による AI agent 向けの位置指定型推論と、明示queryによる同一source内の型由来説明
- Rails / gem DSL plugin
- Sorbet `sig` / `.rbi` の実験的な補助
- wasm playground（[Playground](https://pvcresin.github.io/tyda/playground/)）

## Ruby 推論

主な対応範囲は次のとおり。

- class / module / method / mixin / constant / local / instance variable（LSP / Playground の定義・参照 hover を含む。宣言済み引数の定義位置も対象）
- method dispatch、visibility、`super`、singleton method、refinement（`include` / `prepend` /
  `extend` の適用順を反映。`class << self` 内の `include` は `extend` として出力。有効な refinement は Array / Hash の literal shape や subclass にも祖先順で適用する。静的に完全な `Module#ancestors` は順序付き `Tuple`。トップレベルの
  local receiver も呼び出し時点の型で解決し、型は解決済み receiver の宣言（RBS / source）か plugin からだけ決まり、untyped receiver の呼び出しは `to_s` や `==` を含め常に untyped（`untyped | nil` で `NilClass` にないメソッドも同じ）。メソッド名から戻り値を推測しない。`super(...)` の引数転送と method call の keyword splat から optional keyword 型を広げ、明示的な `super` に渡す `&block` には親 method の既知の block contract を反映する。名前付き `**rest` は個別の keyword 引数を除いた caller record から推論し、動的な keyword hash でも標準 Hash API の key 型（`keys` は `Symbol`）を保つ。`obj[key] = value` は RHS を返し、`obj.[]=(...)` はメソッドの戻り値を保つ。`alias_method` は alias 元の keyword signature と戻り値参照を保持し、source method return refを含む union receiver の引数なし call も後段でarmごとに解決する）
  `untyped` の腕を持つ union receiver への呼び出しは内部で腕ごとに解決し（`untyped | Pathname` の `join` は内部で `untyped | Pathname`）、`nil` の腕は dispatch しない。RBS・hover・code lens では、`untyped` の腕を含む union は RBS の意味どおり `untyped` と表示する。
- block / Proc / lambda / `yield` / Enumerable / Enumerator / Lazy（同じdispatchのsource overrideがない `block_given?` / 安定した `&block` のガードとcall block内のyieldからrequirednessを推論し、明示RBSのblock契約を優先。`Enumerator::Lazy.new` は既知 enumerable source の要素型を block に渡し、yielder への送信型から出力要素型を推論。遅延解決された source method return を
  block dispatch 前に反映し、source method の yield 型に呼び出し引数を代入し、block signature にも positional / keyword method parameter の型を反映し、`&` で渡された Proc / lambda の戻り型にも yield 引数型を反映し、tuple の要素 union とリテラル演算を block 内へ伝播。既知の空配列に対する
  `map` / `collect` / `map!` は空の結果を保ち、source-defined `map` / `collect` override の戻り型を優先し、遅延 block return でも標準 `map` / `collect` の Array shape を維持し、
  単一要素配列リテラルを identity block で `map` / `collect` した場合も要素 shape を維持。source method の yield summary は early-exit guard と分岐内の local 代入を反映し、
  loop 内 yield は条件による local の絞り込みを反映し、statement 位置の pre-test `while` で条件全体が括弧付き単一 local 代入の場合に限り body を絞り込む。複合条件内の代入、式位置の loop、post-test loop は対象外。`for` は source-defined `each` の yield 型を
  index に反映し、単一 yield の複数 block 引数への destructuring も tuple / Array の形に沿って扱う。
  全経路で yield が保証される場合だけ本体が必ず実行されると扱い（source method が位置引数で分岐する場合は、その引数に渡されたリテラルの真偽と `when` の一致で経路を選ぶ）、そのblock末尾のnon-local `return` 後は呼び出し後の処理を到達不能とする。Array / Tuple の `sort_by(&:method)` は要素型を保つ。yield 型が不明な
  保存された Proc / lambda の callsite では、確定した receiver 型の標準 RBS method を読み込み、引数推論に反映する。Proc / lambda リテラルの `call` / `.()` / `[]` は source method と同じ引数束縛（optional の既定値、rest、post、keyword と既定値、`**rest`、block 引数、keyword を受けない parameter への Hash 化）で戻り値を求め、proc は単一の配列引数を展開し、lambda 内の `return` / `break` は lambda の値にする。local に保持した lambda の呼び出しは、本体で代入する外側 local をその文の後に join する。yield 型が不明な
  block 引数は `untyped` にする）
- Array / Hash / Set / Tuple / Record の要素型と shape。unknown な値を `Kernel.Array` に渡した結果は `Array[untyped]` を保ち、unknown な添字の Array index は `untyped` を保つ。型引数のない `Hash` の iteration も key/value を別々の `untyped` block 引数として扱い、keyword-rest の caller record が持つ optional key を保持して `Hash#[]` の `nil` と `Hash#fetch` の fallback / block を反映する。`Hash#delete` は既知のvalue型に `nil` を加え、value型がunknownなら `nil` だけに誤縮退しない。Record の `Hash#fetch` / `Hash#fetch_values` は欠落キーと fallback を反映し、`Array#clear` の呼び出し結果は空 Tuple にし、instance variable の共有要素型は維持
- ループ後の到達性はリテラル条件だけで判定する（推論型では後続コードを捨てない）。抜けられない `while` / `until` / `for` の後は戻り値に含めず、ループの値は `break` 値と正常終了の値の union にする。rescue の先頭が本体の唯一の `raise ... if P` と同じ `retry if P` なら、その rescue は値を返さない
- if / case / rescue / safe navigation / pattern matching の flow narrowing。`is_a?` / `kind_of?` / `instance_of?` と標準 `Class#===` はclass constantをlexical scopeで解決して条件の型を絞り、既知の親型は判定対象の既知の子型まで絞る。`is_a?` / `kind_of?` と標準 `Class#===` では、クラスが分かっているgeneric型の型引数が `untyped` でも外側の継承関係で判定する。双方の継承関係が明確に不一致な分岐は到達不能として扱う。継承関係が不明な既知型は安全側に保ち、標準のclass predicateでunknown inputを検査した真側は対象クラスまで絞る。有効なrefinementがpredicateを変える場合は分岐を絞らない。静的に到達不能な `if` 分岐は推論せず、別メソッドから集約した型だけではinstance variableの分岐を到達不能と判定しない。結果が静的に決まる no-arg `empty?` による分岐も絞り、終了分岐の local を後続へ混ぜず、片側だけで代入された local は未代入経路の `nil` を含める。標準の `Class#===` による postfix の local 代入条件や、pure な no-arg ivar reader の再呼び出しも扱い、安定した primitive 型で分岐先を区別できる場合は、片側だけで初期化された local を含む相関も反映。標準RBSの `nil` / `true` / `false` literal pattern と class pattern は一致する型と残りの型を分け、source-defined case equality ではクラス型による絞り込みをしない。ただし `Const === x` の呼び先が同じファイルの source `===` で、本体が唯一の必須引数を絞る単一の式なら、その条件の絞り込みを呼び出し側の引数へ移す（`case` / `in` と複数ファイルにまたがる定義は対象外）。閉じた型の rightward assignment は成功後の subject local を一致型へ絞る。括弧で囲んだ pattern predicate の capture は真側と直接代入後の local に反映し、不一致時は `nil` または既存値を保つ。裸の `rescue => e` は静的に分かる `raise` / `fail` の StandardError subclass を反映し、不明な raise は `StandardError` にする。ASCII の単純な literal regex と literal string の `=~` は分岐を絞り、option / metacharacter / 複数capture / 動的な文字列は unknown にする。optional parameter の default だけが確定している場合は参照を早期解決せず、snapshot replay で parameter / method return / variable reference を解決してから truthy / falsy projection を適用する。truthy 条件が `bot` になる receiver は missing-method 診断の対象にしない数値 literal の pattern と `==` は Integer と Float をまたいで数値として比較し、`==` の真側は subject の数値 class を保つ。
- `if` の単一 `&&` 条件で、左辺が local read、右辺が直接の local assignment の場合は、短絡時の未代入経路と右辺代入値のtruthy / falsy projectionを分岐 local に反映する。guardと代入先が同じlocalの場合も分岐ごとの値を保つ。右辺内で別localを変更する副作用はbranch scopeへ反映しない。入れ子の複合条件や別の左辺 / 右辺構文は対象外
- optional parameter の `nil?` / `== nil` guard 後は、遅延参照から `nil` だけを除き、`false` は残す
- 標準の no-block `Array#any?` と `Array#all?(Class)` を同じ `&&` 条件で確認した直後は、同じ local Array / `Hash#values` の `first` を非空かつ class 型で絞る。block・source override・refinement を含む場合や、間に別の文がある場合は絞らない
- `Array[T]` の local に対する標準の `size` / `length` の比較・`!empty?` guard と truthy な `values[i]` 条件は、要素 fact として残す。範囲内の literal index の `[]` は `T`（truthy 条件の添字は `nil` / `false` を除く）、範囲内から始まる `[start..]` / `[start, length]` は `Array[T]` にする。local の別用途・再代入・任意の call / `yield` / `super` を含む文や source override・singleton method がある場合は fact を捨てる
- 多重代入、右辺型を使った RBS operator overload の選択と source-defined operator を優先する operator-write、method 内の instance variable 再代入（literal を receiver にする引数・block なしの call の結果も含む。式が読む fact は末尾の call より先に評価される）、文字列・シンボル補間（静的な literal / union の展開を含む）
- Thread / Fiber / Queue の bounded な値伝播。Queue は未解決の要素型を既知の sentinel 要素で上書きしない
- 呼び出し側の引数から parameter 型を集める。`untyped` だけの引数は型を加えず、`untyped` と `nil` の union は `untyped` のまま保ち、到達不能な分岐の `bot` 引数は加えない
- 静的に名前を求められる `alias` / `define_method` / `attr_*`（Record / Tuple の静的 iterator 内で束縛された属性名、静的に判定できる `next if` / `next unless`、束縛やDSLに影響しない条件付き代入を含む。receiver上でiterator methodが上書きされている場合は展開しない。生成 writer の戻り値は引数型に一致し、代入値は対応する reader にも反映） /
  `Struct.new` / `Data.define`（位置引数・キーワード引数の `new` 呼び出しからmember型を推定） / `Forwardable` / `send` / `const_get`

実行時にしか決まらない名前、object identity、method surface は推測せず `untyped` にする。
`dup` などのidentity methodをsource側で上書きしている場合は、built-inのidentity fallbackよりsourceの戻り値を優先する。
型の深さ・union・collection shape には上限がある。

## 型情報ソース

### RBS

- `.rbs` の自動発見、stdlib の lazy load、直接実行される文字列リテラル `require` のsource orderに応じた stdlib feature RBS の読み込み（`io/console` など slash 区切りfeature名も対応）、generic、overload、interface、alias。stdlib feature が core class に追加し core RBS にないメソッド（`pp` の `Kernel#pretty_inspect`、`abbrev` の `Array#abbrev`、`shellwords` の `String#shellescape` など）は、その `require` より後に実行される呼び出しだけで解決する。`shellwords` の `String#shellescape` / `String#shellsplit` は RBS stdlibを使えない場合も補う。メソッド本体内のリテラル `require` は同じメソッド内の後続呼び出しにだけ適用する。class / module / singleton class body内の `require` は、singleton `require` の再定義や動的な `send` / `eval` がある場合はロード済みと推定しない。メソッド内の条件分岐の本体・block / lambda・rescue 内の `require` と動的な `require` もロード済みと推定しない。トップレベルの `require` 再定義ではfeatureを読み込まず、`require` のsource定義やaliasがある場合はメソッド本体内のrequireも推定しない
- inline RBS（`#:` / `# @rbs`）の単一・複数行宣言、block signature、type alias
- `self` / `instance` を call site の受け手へ解決する factory 型

### Sorbet（実験的）

`sorbet/config` がある project で、`sig`、`T.let`、`T::Struct`、`T::Enum`、`.rbi` と
対応する comment extension を読み込む。RBS と同じ安定性や網羅性は保証しない。
`&blk` に対応する sig param の `T.proc...` はメソッドの block 型（`{ (Integer) -> String }`）、
`T.nilable(T.proc...)` は省略可能な block（`?{ ... }`）として扱う。

## Diagnostics

型推論が主であり、diagnostics は補助である。権威ある宣言に対する確実な不一致だけを報告する。

| 種類 | 既定の severity | 方針 |
| --- | --- | --- |
| `argument_type_mismatch` | error | 宣言 param と actual が確実に不一致のときだけ。分岐内のインスタンス変数や `File.exist?` で確認したパスも、その分岐で絞られた型で判定する。Union receiver は各候補型の権威ある署名で調べ、不一致の候補型を message に含める |
| `return_type_mismatch` | error | inline RBS、`@rbs`、Sorbet `sig`、外部RBS/RBIの戻り値型と、メソッド本体の到達可能な返却型が確実に不一致のとき。`untyped` や未解決型を含む不確かな結果は報告しない |
| `arity_mismatch` | error / experimental | Union receiver は候補型と権威ある署名が分かる場合に、引数個数・必須キーワードの不一致を候補型ごとに報告する。ソース定義の `initialize` を標準の `Class#new` 経由で呼ぶ場合は必須引数の不一致を error にする。祖先解決では `::` による絶対指定を保つ。`Struct.new` 由来の constructor は member の省略を許し、上限超過だけを報告する。`Data.define` 由来は、member 名が静的に分かり独自 `initialize` がない場合に限り、必須 member 数と位置・キーワード引数の規則で判定する。それ以外は `TYDA_EXPERIMENTAL_CHECKS=1` のときだけ |
| `missing_record_key` | warning | 形状が分かる非空 record のすべての候補にリテラルキーが存在しないとき。空 record は動的な `to_h` の推論結果と区別できないため対象外。`Hash#merge` の引数が `untyped` や未解決型なら、結果を動的な Hash として扱い対象外。record のキー存在確認後は既知のキーへ絞り、`[] ||= ...` 後の同一 receiver・key も既知として扱う。同じrecordを位置引数またはキーワード引数でblock付き呼び出しに渡した場合、calleeがyield前に変更できるため、そのblock内のチェックは対象外。`Hash#[]` は `nil` を返すため warning にする |
| `missing_method` | warning | receiver が既知 class / singleton、または `nil`（`NilClass`）で、その method surface と祖先が完全に既知のときだけ。遅延型はcall siteのsingleton dispatchを保って解決し、class instance variable と instance variable を混同しない。Union receiver は各候補型で欠けている method を報告し、instance variable の集約型に含まれる `nil` も候補にする。安全 navigation の `nil` は候補から除く。bare `Object` / `Class` / `Module`、plugin が解決する method、既知の class-body DSL は対象外。Concern の `class_methods` 内は includer の singleton surface も確認する。ActiveRecord 系ではDBスキーマや宣言RBIから動的なカラム属性を確認できない場合も対象外 |
| `unresolved_constant` | information | receiver 文脈で未定義と証明できるときだけ |

Unknown、`untyped`、動的な record key、開いた `method_missing` 面、未解決の祖先は誤検知を避けて沈黙する。Union の候補型を名前付き class に解決できない場合、引数型チェックはその呼び出し全体を Unknown として扱う。戻り値の判定では `return` / 末尾式 / 分岐 / loop の到達可能な返却型を使い、到達不能な末尾や `bot` は不一致に含めない。
同名の instance / singleton method がある場合、parameter 参照は定義元の dispatch 種別に一致する署名だけで再解決し、反対種別の署名は流用しない。定義元の種別を証明できない局所 singleton method の parameter は Unknown として扱う。
`respond_to?(:method)` の静的な true 分岐は通常の `if` と modifier `if` の両方で診断時にも反映する。独自の responder や `untyped` は Unknown のまま型を絞らない。
詳細は [incomplete-code-policy.md](incomplete-code-policy.md) を参照する。

診断を一行だけ抑制するには、対象式の行末に `# tyda: ignore` を置く。特定の種類だけを
抑制する場合は `# tyda: ignore[missing_method]` のように診断 code を指定できる。現在の
code は `missing_method`、`argument_type_mismatch`、`arity_mismatch`、`missing_record_key`、
`unresolved_constant` などで、CLI・LSP・Playground で同じ書式を使う。コメントは同じ行の末尾に
置いた場合だけ有効で、単独行のコメントが次の行へ影響することはない。対応する診断がない
ignore は `unused_ignore` warning になり、診断が解消したあとに不要な抑制を見つけられる。

`--diagnostics` の JSON Lines 出力は実行間で byte-identical になる。ファイルは
辞書順の走査順（CLI に明示的に渡したパスはその順序を保つ）、ファイル内の各行は
position 順（line/column 昇順）で並ぶため、複数回実行した出力をそのまま diff できる。

### 型カバレッジ

`--coverage` は、compact scan が記録した推論サイトを最終 workspace registry で再解決し、
宣言スロットと合わせて決定的な JSON として出力する。通常の RBS 出力ではサイトを保持しない
ため、明示的に指定した実行だけがこの追加コストを負う。

宣言と参照は意味の違う単位なので、レポートでは分母を分ける。

- `declarations`: Ruby source の method parameter / return / instance variable。外部 RBS
  だけの宣言と synthetic DSL method は含めない
- `references`: 推論中に観測した `value` / `method_call` / `constant_reference`

各グループと種別には次の三段階を出力する。

- `typed`: 型が確定し、入れ子の型にも未解決 marker がない
- `untyped`: サイトは解析経路に結び付いているが、型が `untyped` または deferred
- `unknown`: receiver や constant の結び付き自体を確立できない

型構造が極端に大きい、または深い場合は、報告処理の上限を超えたサイトを安全側に `untyped`
として扱う。これにより coverage の計測自体が入力によって無制限に膨らまないようにする。

`type_coverage_percent` は `typed / total`、`tracking_coverage_percent` は
`(typed + untyped) / total` である。型が `String` から `Integer` に変わるような、両方とも
`typed` の精度差はこのレポートでは扱わない。Performance workflow では既存の全 subject を
base/head の同じ release binary で一度ずつ解析し、解析ファイル数の減少、`typed` / tracked
サイトの減少、`unknown` の増加を失敗として検出する。`untyped` の増加は警告として結果 JSON
と CI log に残す。Perf と同じ matrix 内で実行するため、追加の Rust build や PR comment は発生しない。

## Rails / gem DSL

Rails の ActiveRecord、ActiveModel、ActiveSupport、ActionController、ActionMailer、ActiveJob、
routes、schema と、主要な gem plugin を扱う。対応 gem の正本は `src/inference/plugins/` であり、
個別 gem の一覧をこの文書に重複して持たない。

- schema / `attribute` / relation から model の型を補う。migration と schema の `t.column name, type` も型引数を読み取り、単純な `column IS NOT NULL` check constraint は非 nullable として扱う
- Concern、association（`belongs_to` / `has_one` の optional attribute hash を取る `build_*` / `create_*` / `create_*!` を含む）、scope、enum（plural map method、文字列キーを返すreader、静的に読めるHash/Array/`HashWithIndifferentAccess.new`定数mappingを含む）、delegate などの静的な DSL を展開する。`alias_attribute` は reader / writer / predicate を追加する。`structure.sql` の外部キー先は明示的な `self.table_name` も参照する
- `serialize` は `type:` がある場合だけその型を使い、型指定のないserialized attributeは `untyped` として扱う。明示されたsource/RBS methodの型は保持する
- ActionController の `tag` helper は、引数なしでは `TagBuilder`、生成したタグ名の呼び出しでは `String` として推論する
- ActiveRecord の `find` は単一 ID、ID 配列、複数 ID の overload とモデル／配列の戻り値を出力する
- ActiveRecord relation と association proxy の `count` / `size` / `length` は `Integer` を返す
- ActiveRecord model の `reflections` / `base_class` と instance の `readonly!` は既知 API として扱い、返り値は未解決なら `untyped` にする
- StateMachines は `state_machine` reader、`<attribute>_name`、state predicate、event method を追加する。machine descriptor の内部 API は `untyped` にする
- Devise は `devise` に指定した module に対応する model instance method を追加し、Paperclip は ActiveRecord model の `has_attached_file` から attachment reader を追加する。Rack::Attack が有効な場合、`Rack::Attack::Request` は `Rack::Request` の request method surface を継承する
- `config/routes.rb` から `*_path` / `*_url` helper を生成する。ActionDispatch の命名規則に沿って `namespace` / `scope`（`as:`、動的 segment）/ `resources` / `resource`（`as:`、`only:` / `except:`、`shallow`）/ `member` / `collection` / `new` / `concern` / `concerns` / `direct` を合成し、除外された action の helper は生成しない。`config/routes/*.rb` の `draw` は未対応
- ActionView helper の `dom_id` / `dom_class` は `String`、`truncate` は nil を受けると nil を返すため `String?` を返す
- `Rails.application.configure` の block では、receiver のない呼び出しを application の `self` で推論する
- ActiveJob の `perform_later` と ActionMailer の `deliver_later` は Rails 7.0 以降 enqueue callback の中断で `false` を返し、`perform_now` は perform callback の中断で `false` を返す。Sidekiq の `perform_async` などは client middleware の中断で `nil` を返す。`Sidekiq::Job` を include するクラスは名前が `Job` で終わっても ActiveJob として扱わない
- ActiveSupport の core extension は plugin の戻り値テーブルで補う。引数の型で戻り値が決まるもの（`Array#including` / `in_groups` など）は plugin が名前を宣言した呼び出しだけ引数を推論する。`String#to_date` などは空白だけの文字列で `nil` を返す
- Grape、GraphQL-Ruby、Devise、Doorkeeper、Sidekiq、Draper、AASM などを plugin で補う
- Rake が有効な project では、`String#ext` / `String#pathmap` の戻り値を `String` として扱う。Gemfile の引数なし `gemspec` は root の唯一の `*.gemspec` の gem 名も宣言済み gem として扱う
- GitLab in-tree の Presenter / CurrentSettings / Metrics / EE 拡張を検出する
- Redmine の `acts_as_*` など、ライブラリ内部の実行時登録は展開しない。必要なら手書き
  `.rbs` / `.rbi`、または生成済み定義を入力する

## LSP / Playground

LSP は TypeProf VSCode 拡張が期待する起動・version・request 契約に対応する。

- incremental text sync（full change も受理）
- Hover、CodeLens、definition、typeDefinition、completion
- method hover は呼び出し形に合う RBS overload を選び、推論済みの引数・戻り値から generic と block 型を具体化する。あとで解決する引数（parameter や未解決 receiver の呼び出し結果。nil / truthy guard で絞った値も含む）は、受理する overload の戻り値が `nil` 以外で食い違うなら `untyped` にする（`["e"] * n` は `String` と決めない）
- Playground は入力中の Ruby とユーザー RBS を debounce 解析し、RBS の変更も同じ hover / diagnostics に反映する
- CodeLens は LSP と Playground で、クラスやモジュールの外にある未注釈の `def` にも推論シグネチャを表示する
- Playground の Ruby エディタは Monaco の基本的なカーソル移動・編集、行操作、Find を有効にする。macOS の `Command+Backspace` は行頭まで削除、`Shift+Command+K` は行削除、`Command+F` はエディタ内検索
- diagnostics の publish と workspace refresh
- CLI と共通の `WorkspaceState` / query backend
- Playground の複数行選択を `Ctrl+/`（Windows/Linux）または `Command+/`（macOS）で Ruby 行コメントに切り替え

Playground は同じ LSP 表示経路を wasm で実行し、Ruby と手書き RBS の結果をブラウザで確認する。
右上の `Examples` メニューから、配列・Tuple・mixin・RBS などの短いサンプルに切り替えられる。
サンプルのソースは末尾改行一つで統一する。
Ruby エディタを主領域として PC / 横長画面では左、狭い画面（768px 以下）では上に置き、RBS は補助領域として右または下に配置する。エディタ内の長い行は折り返し、横スクロールは使わない。動的viewportとSafe Area insetにも対応する。

## 主なコマンド

~~~bash
cargo run -- <path>                         # RBS
cargo run -- --diagnostics <path>           # JSON Lines diagnostics
cargo run -- --coverage <path>              # JSON type coverage
cargo run -- --lsp                          # LSP server
cargo run -- mcp                            # MCP server for AI agents
cargo run -- --include-synthetic-dsl-methods <path>
mise run dev                                # playground
~~~

MCP の client 設定と `infer_type_at_position` の入力・出力は [mcp.md](mcp.md) を参照する。

開発・拡張の手順は [development.md](development.md)、テストの追加方法は
[testing.md](testing.md) を参照する。
