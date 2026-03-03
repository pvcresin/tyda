# Documentation Index

このディレクトリには Tyda の現在の契約・判断基準・使い方だけを置く。実装の経緯、
計測ログ、完了した作業、一時的なデバッグメモは残さず、必要なら git 履歴やテストを参照する。

## 目的別の入口

| 知りたいこと | 文書 |
| --- | --- |
| なぜこの設計か | [design.md](design.md) |
| どのコードが何を担当するか | [architecture.md](architecture.md) |
| 何が使えるか | [features.md](features.md) / [capability-matrix.md](capability-matrix.md) |
| どう開発・起動するか | [development.md](development.md) |
| どうテストを追加するか | [testing.md](testing.md) |
| 速度・メモリの基準 | [performance.md](performance.md) |
| 壊れた入力をどう扱うか | [incomplete-code-policy.md](incomplete-code-policy.md) |
| 未完了の課題 | [roadmap.md](roadmap.md) |

## 保守ルール

- タスクに関係する文書だけ読む。README は索引に留める。
- 実装・テスト・文書が同じ契約を示すよう、対応する変更を同じコミットに入れる。
- 新しい説明は既存の節を整理してから追加する。履歴を追記するだけの節は作らない。
- 完了した roadmap 項目と一時的な調査メモは削除する。現在の制約だけを残す。
- 長くなった文書は別ファイルへ重複コピーせず、責務を見直して縮める。

更新先の対応表は [AGENTS.md](../AGENTS.md) を正とする。
