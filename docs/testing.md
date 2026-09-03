# Testing

推論の品質は user-facing な入力と出力で固定し、実装の細部は必要な範囲だけ unit / integration
test で守る。テストの説明は現在のルールだけを置き、追加履歴は残さない。

## テスト層

| 層 | 守るもの |
| --- | --- |
| scenario | Ruby / RBS / RBI / project fixture から期待 RBS への user-facing 契約 |
| unit | 型演算、名前解決、merge、diagnostic 判定、上限などの局所不変条件 |
| CLI integration | 出力形式、file selection、diagnostics、debug option |
| LSP integration | protocol、snapshot、cache、refresh、incremental change |
| robustness | 壊れた入力での panic / hang がないこと |
| performance | 速度・メモリの基準値と大規模 workspace の有界性 |

## 通常のゲート

~~~bash
./scripts/check.sh
git diff --check
~~~

`check.sh` はテストターゲット（lib / bins / 各 integration test / doc）を個別に実行し、
途中で失敗してもすべてのターゲットを走らせたうえで失敗一覧を末尾にまとめて表示する
（fail-fast で後続ターゲットの失敗が隠れないようにするため）。

通常の Ruby scenario は `analysis::analyze_source_for_display` を入口とする完全解決の snapshot
経路を使う。この経路は LSP、playground、詳細 CLI の表示結果と共通であり、推論結果の差分を
入口ごとの実装で吸収しない。RBS / RBI / project fixture など、入力や解析 profile が異なる
scenario はその条件を維持したうえで、同じ core の解析 backend を利用する。

対象を絞るときは次を使う。

~~~bash
TYDA_SCENARIO_FILTER=<substring> cargo test -q --test scenario_runner -- --test-threads=1
cargo test --test <name>
cargo test <module>::<test>
~~~

GitHub Actions では `Test`、`pages`、`Workflow lint` をPRゲートとする。`Test` はLinuxとWindowsの
Rust build / clippy / test、release workflowはgem / VSIXのLinux x86_64・Windows x64・Intel macOS・
ARM macOS package smoke testも確認する。Linux ARM64はrunnerの利用条件が整い次第追加する。

scenario の期待 RBS は whitespace を正規化して比較するが、意味のない出力変更を許容する
ための仕組みではない。出力を変えたときは意図を scenario / CLI test に残す。

## Scenario の形式

1 ファイルは関連する 1 カテゴリ、1 case は 1 つの振る舞いにする。case 名は短い英語で書く。

~~~markdown
# Ruby / method / example

## Infer a literal return

### update

```ruby
def answer = 42
```

### result

```rbs
def answer: -> 42
```
~~~

case heading の直後には optional な YAML config を置ける。

~~~yaml
ruby_version: "3.3"
rails_version: "7.1"
include_synthetic_dsl_methods: false
known_issue: false
~~~

update section では次を使える。

| block / marker | 用途 |
| --- | --- |
| `ruby` | current file。複数 block は project file として扱う |
| `### file: path` | 次の Ruby / SQL block の project path |
| `rbs` | 外部 RBS input |
| `rbi` | 外部 RBI input |
| `routes` | `config/routes.rb` fixture |
| `schema` / `sql` | Rails schema fixture |
| result の `rbs` | 期待する RBS |

project-backed case は `config/`、`db/`、`app/models/` 以下の最小 fixture だけで構成する。
実在プロジェクトをそのまま fixture にせず、挙動が分かる最小コードへ縮める。

## Scenario のカテゴリ

| カテゴリ | 例 |
| --- | --- |
| `ruby/class` | namespace、継承、mixin、singleton、visibility |
| `ruby/control` | narrowing、branch、rescue、loop |
| `ruby/literal` | collection shape、補間、complexity cap |
| `ruby/method` | return、param、block、recursion、dispatch |
| `ruby/runtime` | Thread、Fiber、Queue、runtime fact |
| `ruby/variable` | local、ivar、constant、pattern scope |
| `ruby/rbs_comment` / `rbs_input` | inline / external RBS |
| `sorbet` | comment、sig、RBI、T::Struct |
| `rails/` | DSL、routes、schema、framework plugin |

対応範囲の要約は [capability-matrix.md](capability-matrix.md) に置く。未対応の user-facing
制約だけを roadmap に書き、テスト一覧を roadmap の代わりにしない。

## 外部コードを素材にする方針

外部プロジェクトやツールは、型結果との parity を測るためではなく、Ruby のコードパターンを
見つけるために使う。推論できるパターンは scenario にし、まだ扱えないパターンは安全な
縮退結果を確認するか、対応範囲として記録する。

| 素材 | 収集するパターン |
| --- | --- |
| Ruby / Prism | 構文、AST 境界、壊れた入力、version gate |
| Ruby LSP | workspace、document change、definition、completion |
| Tapioca / RBS | generated RBI、schema、relation、宣言の合成 |
| Steep / Sorbet / TypeProf | comment、signature、generic、LSP の入口 |
| Rigor-type / Method-Ray / Type-Guessr | 実コードの型推定が難しい idiom |
| Redmine / Mastodon / GitLab | Rails DSL、巨大 workspace、in-tree extension |

Redmine の runtime DSL 登録は対象外とし、必要なら RBS / RBI fixture で表す。
GitLab の static extension は [tests/scenarios/rails/dsl/gitlab_presenter.md](../tests/scenarios/rails/dsl/gitlab_presenter.md)、
Redmine の縮退契約は [tests/scenarios/rails/dsl/redmine.md](../tests/scenarios/rails/dsl/redmine.md) に置く。

## LSP と diagnostics

- LSP test は protocol の形だけでなく、CLI と同じ query backend の結果を確認する。
- `didOpen` / `didChange` 後の Hover、CodeLens、diagnostics が最新 snapshot を見ることを固定する。
- workspace scan 完了前の誤診断抑制、refresh の coalesce、full / incremental change を確認する。
- diagnostics は真陽性を先に固定し、Unknown を沈黙させる変更では false positive も確認する。

## Robustness / performance

~~~bash
cargo test --release --test pathological_inputs
cargo test --test mutation_robustness
./scripts/benchmark.sh <path> [runs]
./scripts/benchmark_lsp.sh [runs] [subject] [release|debug]
~~~

mutation test は壊れた入力の graceful degradation、pathological test は深い再帰・巨大 union・
循環・自己参照 shape の有界性を確認する。基準値と計測ルールは
[performance.md](performance.md) に置く。

## 追加時のルール

- まず最小 scenario を追加し、同じ入力を複数のテスト層で重複させない。
- 期待 RBS は user-facing な契約だけを含め、内部用 synthetic method を無闇に表示しない。
- まだ推論できない入力も panic / hang しない結果を固定する。
- 正しい Ruby だが未対応の期待値は `tests/scenarios/known-issues/` に置くか、case config に
  `known_issue: true` を付ける。推論が後から一致したら通常 scenario へ昇格する。
- Ruby version / Rails version による意味の差だけを version case にする。
- DSL は gem / framework ごとに分け、1 case に複数の無関係な機能を詰め込まない。
- source 固有の名前は一般化し、API 名や DSL 名など意味に必要な固有名だけ残す。
- scenario の意図が分からない自由文や作業履歴を追加しない。
