# 不完全コードの扱いポリシー

Tyda はコンパイラではなく型推論器である。壊れた入力でも panic せず、確信のある部分だけを
返し、不明な部分は縮退する。

## 基本原則

1. **必ず部分結果を返す。** Prism の error tolerance と安全ガードで panic、hang、過剰な型の
   増殖を防ぐ。ガードは病的入力向けとし、性能対策として通常コードの推論を制限しない。
2. **被害を局所化する。** 壊れた method だけの Hover / CodeLens / diagnostics を抑制し、
   無関係な method の結果は残す。
3. **Unknown と No を分ける。** receiver、祖先、actual type が不明なら「間違い」と断定しない。
4. **未完成と確実な誤りを分ける。** 前者は `untyped` または沈黙、後者だけ型エラーの診断にする。
   安全ガード発動時は解析制限に到達したことを診断する。

## 入力別の挙動

| 状態 | 挙動 |
| --- | --- |
| 未終端 string など value-corrupting syntax error | 該当 method の Hover / CodeLens / diagnostics を抑制 |
| `end` 待ちなど incremental structural error | source fallback を残し、入力中の flicker を避ける |
| 未定義 class / 未解決 superclass | method surface を推定せず missing method を抑制 |
| 未定義 constant | receiver 文脈で確実な場合だけ information |
| 既知 class の確実な missing method | warning |
| Union receiver の既知候補型に対する missing method | warning。安全 navigation の `nil` は除外し、未知の候補型や開いた method surface があれば抑制。bare `Object` / `Class` / `Module`、plugin が解決する method、既知の class-body DSL は通常の missing method と同じく抑制 |
| Union 候補型の権威ある署名に対する引数型・arity 不一致 | error。引数型は既存の `argument_type_mismatch` 判定、個数は曖昧な keyword / splat / overload を除外 |
| 権威ある param との確実な型不一致 | error |
| 権威ある戻り値型との確実な不一致 | error |
| `untyped`、未解決 ref、開いた `method_missing` 面 | Unknown として沈黙 |
| 病的入力で complexity / fuel / union の安全上限に到達 | 影響部分を `untyped` にし、診断を出す。通常の大規模 OSS では発動させない |

祖先 chain に未解決 edge がある場合、class の method surface は完全とはみなさない。
module の bare call も host が静的に分からなければ診断しない。

## severity

- **error**: `argument_type_mismatch` の確実な不一致
- **error**: Union 候補型に対する権威ある引数個数・必須 keyword の不一致
- **error**: `return_type_mismatch` の確実な不一致。メソッド本体の到達可能な返却値を宣言型と比較し、未知型を含む結果は沈黙する。`bot` の返却は到達不能なので不一致にしない
- **warning**: 完全に既知の class 上の `missing_method`
- **information**: receiver 文脈で確実な `unresolved_constant`
- **バッジ**: syntax error。壊れた入力を波線で過剰に埋めない

単一ファイル CLI は別ファイルの定義を持たないため Unknown が増える。workspace / LSP /
project-backed scenario では context を使って解決する。
