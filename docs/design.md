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
  増分更新・bounded な型表現で速度とメモリを制御する。
- **アドホックな例外を増やさない。** 知識は RBS、宣言的なテーブル、または gem 単位の plugin
  に置き、個別リポジトリ名に依存する分岐を解析コアへ持ち込まない。
- **確信のある情報だけを user-facing に出す。** 不明な型や開いたメソッド面は `untyped` とし、
  診断は「確実に間違い」と証明できる場合に限る。
- **コードパターンは scenario で固定する。** 実装詳細ではなく、Ruby 入力と期待する RBS /
  diagnostics を回帰の正本にする。
- **解析コアを共有する。** CLI、LSP、scenario test の意味論を別々に実装しない。

## semantic backend

長寿命の `WorkspaceState` にファイルの snapshot、依存関係、推論結果を保持し、CLI / LSP /
scenario test が共通の query backend を使う。変更は export が変わった依存先へだけ伝播させ、
request ごとの workspace 全体の再解析・再 merge を避ける。

RBS 出力、Hover、CodeLens、completion、diagnostics はこの query backend の projection であり、
render や LSP adapter を意味論の正本にしない。

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
