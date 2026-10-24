# 設計思想

Tyda は Ruby / Rails の型推論器であり、型チェッカーを主目的にしない。型のないコードから
RBS を投影し、エディタには同じ推論結果を LSP で返す。

## 目的と優先順位

メソッドの型情報を表示・参照解決に使うときは、次の順で選択する。

| 優先度 | 情報源 | 位置づけ |
| --- | --- | --- |
| 1 | inline RBS（`#:` / `# @rbs`） | コード中の最も明示的な補足 |
| 2 | Sorbet `sig` | Ruby source に書かれた明示的な宣言 |
| 3 | `.rbs` | 外部の RBS 宣言。生成・手書きを問わず利用する |
| 4 | `.rbi` | Sorbet の外部宣言。生成定義を含む |
| 5 | Ruby code inference | 明示的な情報源がない場合の基本経路 |

型診断は情報源ではなく、選択された型に対する補助的な検証である。推論結果が確定しないときは
具体型を捏造せず `untyped` に縮退する。

`sig` と inline RBS comment を同じメソッドに書く場合は、`sig` の後、`def` の直前に `#:` を置く。
この既存の構文上の扱いを正規形とし、物理的な並びを全通り受け付けるための解析拡張は行わない。

この優先順位は型情報の選択順であり、Ruby の定義・参照から収集する依存 edge の優先順ではない。
source の annotation を更新・削除した場合は snapshot と fingerprint を更新し、現在残っている情報源から
再構築して下位の情報源へフォールバックする。

## 中核原則

- **大規模 workspace で速く、少ないメモリで動く。** 永続キャッシュは持たず、共有・遅延ロード・
  増分更新で速度とメモリを制御する。
- **Ruby semantics と公式 RBS を優先する。** 一般規則が明確なら最初から広く扱う。公式 RBS の誤りは
  upstream の修正を優先し、修正までは安全に `untyped` とする。個別リポジトリや特定クラス名に
  依存する分岐をコアに持ち込まず、RBS で表しにくい library DSL は plugin に置く。
- **推論品質と性能を同時に守る。** 性能問題はアルゴリズムから解決し、通常コードの推論を落とす
  固定深度上限を性能対策に使わない。病的入力向けの安全ガードは許容するが、通常の大規模 OSS では
  発動させない。ガード時は影響部分を `untyped` にして診断する。
- **簡潔で明確な実装を選ぶ。** 同じ振る舞いなら簡潔に書き、重複が見えたら適切に抽象化する。
  既存実装を正解と決めつけず、今回の変更に密接なアンチパターンは範囲を保てるなら直す。無関係な
  リファクタには広げない。読みやすさやコード量の改善は、compile / runtime の負荷も減らし得る。
  分割は読みやすさと性能を保ち、変更の影響と作業衝突を小さくする。
- **確信のある情報だけを user-facing に出す。** 不明な型や開いたメソッド面は `untyped` とし、
  型の誤りを示す診断は「確実に間違い」と証明できる場合に限る。安全ガードの診断は、解析制限に
  到達した事実を伝える。
- **コードパターンは scenario で固定する。** 実装詳細ではなく、Ruby 入力と期待する RBS /
  diagnostics を回帰の正本にする。
- **解析コアを共有する。** CLI、LSP、MCP、WASM / playground、scenario test の意味論を別々に実装しない。

分岐 join では、互いに重ならない組み込みprimitive型が識別子として存在するとき、厳密なscope mergeで
変更されたlocalの分岐値を一つずつ一時保持する。追跡対象は通常の厳密なmergeが受け持つlocal集合
（現在は最大64個）に従い、相関専用の件数上限は重ねない。追加状態は変更local数に比例し、分岐の直積は
作らない。後続のnarrowingで一致する枝が一つに定まった場合だけ、関連localにもその枝の型を反映する。
候補が重なる、または追跡中のlocalが再代入された場合は相関を捨て、既存のunionを維持する。

`if` / `unless` の両方の分岐から後続へ到達できるとき、一方だけで初めて代入されたlocalは、
もう一方では `nil` として結合する。

IntegerやSymbolなど変更できない組み込み値のlocalとliteralを `==` / `!=` で比較する条件では、分岐で新しく代入されたlocalの型を
保持する。同じ条件を再評価した枝の中だけでその型を戻し、比較対象か追跡localへの書き込みで該当する
記録を破棄する。無関係なlocalの更新では記録を維持し、条件の記録は各scopeで最大64件まで
保持する。これにより、同じ条件を後で確認するRubyコードでnilを誤って診断せず、分岐外ではnilableな
型を保つ。

## semantic backend

長寿命の `WorkspaceState` にファイルの snapshot、依存関係、推論結果を保持し、CLI / LSP / MCP /
WASM / scenario test が共通の query backend を使う。変更は export が変わった依存先へだけ伝播させ、
request ごとの workspace 全体の再解析・再 merge を避ける。

RBS 出力、Hover、CodeLens、completion、diagnostics はこの query backend の projection であり、
render や LSP adapter を意味論の正本にしない。

## 型カバレッジの境界

型カバレッジも render 結果の逆解析ではなく、同じ推論 backend が観測したサイトを最終 registry
で再解決し、宣言スロットと合わせて作る projection とする。Ruby source の method parameter / return / instance variable は
`declarations`、式と method call の観測点は `references` に分け、性質の違うサイトを一つの
分母に混ぜない。

サイトの状態は `typed` / `untyped` / `unknown` の三段階だけを最初の契約とする。`typed` は
入れ子も含めて未解決 marker がなく、`untyped` は解析経路に結び付いているが型が確定せず、
`unknown` は receiver や constant の結び付きを確立できない状態である。型名同士の精密な
比較は推論アルゴリズムの変更でノイズになりやすいため、coverage の初期版では扱わない。
サイトの記録は明示的な opt-in とし、通常の CLI の hot path とメモリ契約を変えない。巨大または
深すぎる型構造は報告処理の上限で安全側に `untyped` とする。

## LSP の境界

TypeProf VSCode 拡張との連携に必要な起動、version、request / notification の契約は LSP adapter
で維持する。一方、TypeProf / Steep / Sorbet の推論結果との互換性は目標にしない。推論の user-facing
な契約は Ruby semantics と Tyda の scenario で定める。

## framework / gem DSL

Rails や gem の DSL は、library-scoped に有効化される 1 gem 1 plugin として実装する。
plugin は安定した `PluginCx` 越しに必要な操作だけを行い、未解決結果より具体的な型を増やせる場合に
限って synthetic API を追加する。

ライブラリ内部の実行時登録をプロジェクト固有のソース走査で再現しない。Redmine の `acts_as_*`
など動的に追加される API は対象外とし、必要な型は手書きの `.rbs` / `.rbi`、または Tapioca
相当の生成済み定義を入力する。

## 診断の線引き

診断の判定は Yes / No / Unknown の三値で扱う。No だけを error / warning として報告し、
Unknown は沈黙または `untyped` にする。詳細な severity と壊れた入力の扱いは
[incomplete-code-policy.md](incomplete-code-policy.md) を参照する。
