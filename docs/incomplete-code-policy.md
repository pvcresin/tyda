# 不完全コードの扱いポリシー

Tyda はコンパイラではなく型推論器である。壊れた入力でも panic せず、確信のある部分だけを
返し、不明な部分は縮退する。

## 基本原則

1. **必ず部分結果を返す。** Prism の error tolerance、再帰 guard、型の complexity cap で
   panic、hang、過剰な型の増殖を防ぐ。
2. **被害を局所化する。** 壊れた method だけの Hover / CodeLens / diagnostics を抑制し、
   無関係な method の結果は残す。
3. **Unknown と No を分ける。** receiver、祖先、actual type が不明なら「間違い」と断定しない。
4. **未完成と確実な誤りを分ける。** 前者は `untyped` または沈黙、後者だけ diagnostics にする。

## 入力別の挙動

| 状態 | 挙動 |
| --- | --- |
| 未終端 string など value-corrupting syntax error | 該当 method の Hover / CodeLens / diagnostics を抑制 |
| `end` 待ちなど incremental structural error | source fallback を残し、入力中の flicker を避ける |
| 未定義 class / 未解決 superclass | method surface を推定せず missing method を抑制 |
| 未定義 constant | receiver 文脈で確実な場合だけ information |
| 既知 class の確実な missing method | warning |
| 権威ある param との確実な型不一致 | error |
| `untyped`、未解決 ref、開いた `method_missing` 面 | Unknown として沈黙 |
| complexity / fuel / union 上限超過 | `untyped` へ縮退 |

祖先 chain に未解決 edge がある場合、class の method surface は完全とはみなさない。
module の bare call も host が静的に分からなければ診断しない。

## severity

- **error**: `argument_type_mismatch` の確実な不一致
- **warning**: 完全に既知の class 上の `missing_method`
- **information**: receiver 文脈で確実な `unresolved_constant`
- **バッジ**: syntax error。壊れた入力を波線で過剰に埋めない

単一ファイル CLI は別ファイルの定義を持たないため Unknown が増える。workspace / LSP /
project-backed scenario では context を使って解決する。
