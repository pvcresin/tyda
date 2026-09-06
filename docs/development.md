# Development Guide

人間と AI のコントリビュータ向けの最小手順。設計は [design.md](design.md)、構造は
[architecture.md](architecture.md)、未完了項目は [roadmap.md](roadmap.md) を参照する。

## 初回セットアップ

ツールの版は mise と `rust-toolchain.toml` で固定する。

~~~bash
mise trust
mise install
mise run setup
~~~

`setup` は stdlib RBS の展開、npm 依存、Playwright browser を準備する。生成物の
`vendor/rbs/`、`target/`、`node_modules/` は source of truth ではない。

## よく使うタスク

~~~bash
mise tasks
mise run test
mise run check
mise run fmt
mise run clippy
mise run dev
mise run e2e
~~~

`./scripts/check.sh` は lockfile に合わせた stdlib RBS の生成確認と、Rust の format、clippy、test、release build をまとめて実行する。
wasm feature の clippy と playground の整形確認も含め、変更の完了条件はこの check とする。

実験的な arity diagnostics は次で確認できる。

~~~bash
TYDA_EXPERIMENTAL_CHECKS=1 cargo run -- --diagnostics <path>
~~~

## RBS と parser

- stdlib の RBS は Gemfile / Gemfile.lock の rbs gem から `mise run vendor-rbs` で生成する。
  `./scripts/check.sh` も生成 version marker を確認し、古い `vendor/rbs/` を自動更新する。
- `crates/rbs-sys` は公式 RBS C parser への薄い FFI。型定義の変換後に parser の構造体を保持しない。
- Ruby runtime は build-time のみで、生成済み CLI / wasm の実行には不要。
- RBS / RBI の import を変更したら、対応する scenario と外部型の unit test を更新する。

## 変更の進め方

1. 変更の user-facing な契約を scenario、CLI test、LSP test のいずれかで先に特定する。
2. 既存の query backend / plugin / 宣言テーブルに合流できる設計を選ぶ。
3. 実装、回帰テスト、対応する living document を同じ変更に含める。
4. `git diff --check` と `./scripts/check.sh` を実行する。

個別 repository の名前に依存する分岐や、一時的な debug API は追加しない。DSL が runtime に
登録されるだけで静的に証明できない場合は、外部 RBS / RBI の入力を選ぶ。

## CI

- PR の基本ゲートは `Test`、`Performance`、`pages`、`Workflow lint`。`Test` は Ubuntu の format / lint / test shards、Windows の Rust build / clippy / test、VS Code 拡張の型検査・bundleを確認し、`Performance` は pinned な Ruby / Rails OSS subject の速度・max RSSを base/head で比較する。Performance は binary を一度だけ build して subject ごとの matrix jobへ配布し、`pages` は wasm build と E2E を確認する。
- PRでは各workflowが `scripts/ci/classify-changed-paths.sh` で変更範囲を分類する。Markdownと `playground/**` だけの変更では汎用Rust・性能・VS Code CIをjob-levelでskipし、Playgroundのコード変更時は `pages` の wasm build + E2Eを実行する。workflow自体は起動するため、required checkがPendingのまま取り残されない。
- `Test` は Linux と Windows の Rust build / clippy / test を確認する。
- release workflow は VSIX packaging と smoke test、main マージごとの platform gem packaging / smoke test / RubyGems Trusted Publishing を確認する。gem 公開後は同じバージョンの `v...` tag と GitHub Release を作成し、前回 Release 以降のマージPRを自動生成ノートに記録する。RubyGems 側の pending trusted publisher を事前に設定する。Linux ARM64 はGitHub-hosted runnerの利用条件が整い次第追加する。
- Actions は commit SHA で固定し、`Workflow lint` の `actionlint` で workflow の構文・context を検査する。

Windows のローカル開発は、現行の `scripts/*.sh` と `mise` task が Bash 前提のため Git Bash または WSL を使う。配布物はWindows x64をrelease workflowで検証する。

## コーディング規約

- Rust、コメント、root の文書、commit は既存の英語規約に合わせる。`docs/` の説明は日本語。
- コメントは「何をするか」ではなく、コードから分からない理由だけを書く。
- 型推論の不確実性は `untyped` / Unknown に倒し、誤った具体型を返さない。
- 公開出力（RBS、diagnostics、LSP）の変更は byte / protocol 回帰を確認する。
- docs は現在の状態だけを保つ。完了した作業履歴は追記しない。

## VS Code / Cursor

Tyda は `tyda --lsp` が出力する TCP 接続情報で LSP server を提供する。TypeProf 拡張を使う
場合は `typeprof.server.path` に Tyda binary を指定する。protocol 起動と version 契約は
LSP compatibility のため維持するが、推論結果を TypeProf と一致させるものではない。

~~~bash
mise run vscode-deps
mise run vscode-build
mise run vscode-package
~~~

package task は platform binary と stdlib RBS を `vscode/` に同梱して VSIX を作る。
生成物は commit しない。release version は `tyda-version.txt` と release workflow を正とする。
