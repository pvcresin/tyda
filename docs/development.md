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
mise run build-pages
~~~

Rust の日常的な反復では、まず `mise run check-quick`（全targetの型確認）を使う。
実行可能な最適化バイナリが必要なときは `mise run build-fast` を使う。このタスクと
`./scripts/check.sh` は、実行性能を重視する `release`（thin LTO、codegen 1）ではなく、
LTOを無効にしてcodegenを並列化した `release-fast` を使う。配布物、性能計測、package
workflowは引き続き `release` を使う。

`./scripts/check.sh` は lockfile に合わせた stdlib RBS の生成確認と、Rust の format、clippy、test、release-fast build をまとめて実行する。
`mise run check` はこれに加えて playground の typecheck、oxlint、oxfmt check も実行し、変更の完了条件とする。

クリーンなreleaseビルドやCI以外の再ビルドでは、任意でsccacheを使える。macOSは
`brew install sccache`、その他の環境は各OSのパッケージまたは `cargo install sccache --locked`
で導入し、次のように実行する。

~~~bash
CARGO_INCREMENTAL=0 RUSTC_WRAPPER=sccache cargo build --release
sccache --show-stats
~~~

通常の `cargo build` / `cargo check` はincremental compilationが有効なdev profileのまま
使うほうが速い。sccacheはincrementalなRust crateをキャッシュしないため、常用する場合も
このrelease/clean build用の指定に限定する。CIでは同じ設定を自動で適用し、GitHub Actions
backendへコンパイラ成果物を保存する。

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

- PR の基本ゲートは `Test`、`Performance`、`Analysis compatibility`、`pages`、`release-vscode`、`Workflow lint`。`Test` は Ubuntu の format / lint / test shards、Windows の Rust build / clippy / test、VS Code 拡張の型検査・bundleを確認し、`Performance` は pinned な Ruby / Rails OSS subject の速度・max RSSを base/head で比較する。`Analysis compatibility` は base/headそれぞれのTydaと対応するRBSを組み合わせ、OSS subjectのRBS出力、diagnostics、coverageを比較する。Performance は binary を一度だけ build して subject ごとの matrix jobへ配布し、`pages` は wasm build と E2E を確認する。
- PRでは各workflowが `scripts/ci/classify-changed-paths.sh` で変更範囲を分類する。`docs/**` または `playground/**` だけの変更では汎用Rust・性能・VS Code CIをjob-levelでskipし、`docs/**` の変更では `pages` のドキュメント build、Playgroundのコード変更時は wasm build + E2Eを実行する。PRゲートworkflowは常設のaggregate gateを必ず生成し、対象外のときは分類成功と依存jobの全skipを確認してsuccess、対象内のjobの失敗・想定外skip・分類失敗はfailureとして報告する。matrix個別jobは結果とログのために残すが、required checkにはしない。`approved-analysis-change` ラベルを付けたときだけ、MaintainerまたはAdminが付けたことをGitHub APIで確認したうえで、解析出力・coverageの意図した差分を許可する。新しいcommitが積まれた場合はラベルを自動削除し、再確認を要求する。
- `Test` は Linux と Windows の Rust build / clippy / test を確認する。Windowsも全test targetを実行し、
  unit、軽量integration、mutation、pathological、docのshardに分けて独立したrunnerで並列化する。
  高コストなtest target同士を同じrunnerで競合させない。`test` gateはLinuxのformat / lint / test、Windowsの `windows` gate、VS Code checkの結果を集約し、`windows` gateはWindowsのformat / clippyと全shardの結果を集約する。
  Rust compile jobは
  `CARGO_INCREMENTAL=0` とsccacheのGitHub Actions backendを使い、`rust-cache`はCargoの
  registry/gitだけを保存する（target directoryとの二重キャッシュを避ける）。
- `pages` はPlaygroundの `format:check`、typecheck、oxlintを明示的に通した後、VitePressのドキュメント、wasm、RBS bundle、Playgroundをbuildして `pages-dist/` に組み立て、`/play/` のPlaygroundに対してE2Eを実行する。`pages` gateが `e2e-test` の結果を集約するため、matrixではない安定したcheckだけをauto-mergeの保護対象にする。
- GitHub Pagesは `main` へのpush時だけ `pages-dist/` を公開する。トップの紹介ページは `docs/index.md` から生成し、ドキュメントは `/docs/`、Playgroundは `/play/` で提供する。`docs/README.md` は `/docs/` の索引として表示する。
- `/play/` はVitePressのSPAルートではなく同じPages artifact内の静的アプリなので、トップとナビゲーションのリンクには `target: "_self"` を指定してフルページ遷移させる。
- Pages のE2Eは `pages-dist/` を `/tyda/` 配下でも解決できるローカル静的サーバーで配信し、project siteの実URLに合わせてTop・Docs・Playgroundの遷移、reload、Back/Forward、hash復元を確認する。
- Dependabot は Bundler、root / `vscode/` の npm、Cargo、GitHub Actionsを週次で更新する。Major更新はPRを作るが自動mergeせず、Major以外と手動PRは全required checkが通った後にGitHubのauto-mergeへ登録する。auto-merge workflowは `main` がbranch protectionで保護されていない場合は登録を拒否する。手動PRでworkflowや `scripts/**` などCIポリシーを変更した場合はauto-mergeを登録せず、Maintainer/Adminのレビューと手動mergeを要求する。
- 依存更新で脆弱性が見つかった場合は、direct / transitive、runtime / development-only、実際の到達経路と生成物へのバンドル有無を確認する。まず直接依存または親依存の正規の更新で修正版を取り込めるか検証し、解決できる場合だけ対応する。`overrides` や `resolutions` は使用しない。正規の更新で解決できない場合はライブラリ側の対応を待ち、脆弱性は無理に解消扱いにせずDependabot alertをopenのまま保持する。上流修正版がリリースされたら、通常の依存更新として再評価する。
- release workflow は VSIX packaging と smoke test、main マージごとの platform gem packaging / smoke test / RubyGems Trusted Publishing を確認する。`release-vscode` のPR buildは同じPRの新しいcommitで古い実行をcancelするが、tag pushとgem公開のmain workflowは公開途中の実行をcancelしない。gem 公開後は同じバージョンの `v...` tag と GitHub Release を作成し、前回 Release 以降のマージPRを自動生成ノートに記録する。RubyGems 側の pending trusted publisher を事前に設定する。Linux ARM64 はGitHub-hosted runnerの利用条件が整い次第追加する。
- `main` の検証workflowはpush時にも変更範囲を分類し、READMEなどPagesに影響しないMarkdownだけならTest / Performance / VS Codeのfull CIを起動しない。`docs/**` や `playground/**` はPages buildを維持し、依存ファイル、Rust / Ruby / TypeScript、workflow、`scripts/**` の変更はfull CIを維持する。`release-gem` は現行の「main mergeごとに公開する」仕様のため、この分類ではskipしない。
- Actions は commit SHA で固定し、`Workflow lint` の `actionlint` で workflow の構文・context を検査する。

`main` のbranch protectionでは、matrix個別jobではなく、常に報告される `test`、`performance`、`analysis-compatibility`、`pages`、`release-vscode`、`actionlint` のaggregate gateだけをrequiredにする。各gateは対象scopeでは全依存jobのsuccessを要求し、対象外scopeでは分類成功と依存jobの全skipを確認した場合だけsuccessとして扱う。workflow変更をmainへ反映した後、required contextsもこの一覧へ更新する。依存更新PRだけでなく手動PRも同じauto-merge対象だが、解析結果を意図的に変更する場合は差分をJob Summaryとartifactで確認してから、MaintainerまたはAdminがラベルを付ける。

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
