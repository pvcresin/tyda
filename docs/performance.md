# Performance

速度とメモリの基準値だけを記録する。過去の測定値、原因調査、最適化の経緯は git 履歴に残し、
この表には現在の基準だけを置く。

## 計測コマンド

subject は `scripts/setup_subjects.sh` が固定コミットで取得する（`--list` で
pin 一覧、引数で個別指定）。redmine と gitlab は解析対象のディレクトリだけを
sparse checkout する。合計約440MB、取得は数分。表の値は同じ pin での計測なので、
subject を更新したら基準値も測り直す。`subject/sample` だけは repo 同梱。

### CLI

~~~bash
./scripts/setup_subjects.sh              # 固定コミットで subject を用意
./scripts/benchmark.sh <path> [runs]
/usr/bin/time -l target/release/tyda <path> >/tmp/tyda_cli.out
~~~

速度は複数 run、メモリは max RSS を記録する。大規模 Rails は
`subject/gitlab/app` のように対象範囲を明示する。

`TYDA_PRELOAD_TIMING=1` で preload の内訳（stdlib index / user RBS・RBI / rails schema・
routes / DSL 検出 / pool build）を、`TYDA_MEMORY_BREAKDOWN=1` で projection 後の registry の
deep byte 内訳（method body / call site / container / constant+ivar / resolve_params_cache /
stdlib lazy loader / 保持中の file source / sym interner）を stderr に出す。checkpoint 行は
max RSS（`rss=`）と実測 RSS（`live=`）を併記する。両方 off のときは env var の存在チェック
のみでホットパスに影響しない。

### LSP

~~~bash
./scripts/benchmark_lsp.sh [runs] [subject_path] [release|debug]
TYDA_LSP_BENCH_ROOT=subject/gitlab/app cargo test --release bench_initialize_analysis_mastodon_scale -- --nocapture
cargo test --release bench_workspace_rescan_mastodon_scale -- --nocapture
~~~

正式値は release build、原則 3 runs。workspace scan、first / cached / dirty display、max RSS を
分けて見る。initialize の cold path と通常 display は別の基準にする。

## CI の性能ゲート

`.github/workflows/performance.yml` は pinned commit の `subject/gitlab/app` と
`subject/optcarrot` を matrix の別 runner で計測し、同じ runner 上で base と head を交互に計測する。
pull request は 3 回、main push と手動実行は 5 回とし、CLI の全体解析と LSP の workspace scan を対象にする。
各 run の最大 RSS も同時に取る。解析 worker 数は 2 に固定し、大きな subject は並列化せず `nice -n 19` で実行する。
今回のように optcarrot の base がまだ timeout する場合は、同 subject のみ base timeout を30秒の比較上限として
扱う。head がその上限内に収まらない場合は失敗する。

両variantのrelease binaryは1つのCargo target directoryで順にビルドしてから退避する。LSPはテスト
harnessを再ビルドせず、同じrelease binaryを軽量なLSP clientから駆動するため、base/head間で依存crateの
コンパイル成果物を共有できる。Perf job専用のRust cacheにはworkspace crateを含むこのtarget directoryも保存する。

手元で同じ比較を行う場合は、subject と vendor/RBS を用意したうえで次を実行する。

~~~bash
TYDA_PERF_BASE_REF=origin/main ./scripts/benchmark_ci.sh
~~~

単一 run の揺れで失敗しないよう中央値で比較し、時間は 15% 超、メモリは 10% 超で warning を出す。
時間が 30% 超かつ 100ms 以上、または max RSS が 20% 超かつ 16MiB 以上増えた場合だけ CI を失敗させる。
小さな劣化を許容しつつ、実質的な回帰は PR の段階で止めるための初期値である。基準を別 runner の
過去値と比較せず、base/head を同じ job で測ることで CPU や runner の世代差を打ち消す。

GitHub の branch protection では、この workflow の `large-app (gitlab)` と `large-app (optcarrot)` check を
required に設定する。

結果は subject ごとに `target/performance/<subject>/result.json` として artifact に保存する。warning が継続する場合や runner
環境が変わった場合は、まず複数回の結果を確認してから閾値を見直す。性能計測の対象を追加するときも、
同じ測定順序・worker 数・subject pin を維持する。

計測のプロセス監視と結果比較は Ruby の `scripts/measure_process.rb` と
`scripts/compare_performance.rb` で行い、リポジトリの開発用 Ruby 環境を共有する。

## 現在の基準値

### CLI

| subject | 規模 | elapsed | max RSS |
| --- | --- | ---: | ---: |
| `subject/sample` | 小 | 0.03s | - |
| `subject/rack` | 小 | 0.08s | - |
| `subject/rake` | 小 | 0.05s | - |
| `subject/rubygems` | 中 | 3.0s | 122MB |
| `subject/mastodon` | 中 | 0.88s（3 runs、2026-08-27） | 147–161MB |
| `subject/gitlab/app` | 大、6,455 files | 1.84s（6 runs 中央、2026-08-27、compact-scan 0.75s） | 283–297MB（6 runs、range） |
| `subject/optcarrot` | 小、42 files | 0.30s（3 runs 中央、2026-09-05、worker 2） | 54–61MB（3 runs、range） |

`--diagnostics` は targets に加えて workspace context を読むため、通常の RBS render と別軸で
比較する。

### LSP

| subject | workspace scan | rescan | first display | cached / dirty display | scan 後 RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| `subject/gitlab/app` | 0.44–0.50s（3 runs、2026-08-27、6,458 files） | 5ms（no-op、6,458 known files） | 1–2ms | 0–2ms | 177–184MB（phys footprint、5 runs range） |
| `subject/optcarrot` | 0.45s（3 runs、2026-09-05、42 files、worker 2） | - | - | - | 53–58MB（3 runs、range） |

subject、build mode、run 数が違う値を同じ行へ混ぜない。手元の非公開プロジェクトを
subject にする場合は `TYDA_EXTRA_SUBJECT` で指すことができるが、再現できない値は
この表に載せない。

`bench_memory_breakdown_mastodon_scale` の `[true]` 行による gitlab/app の RSS 帰属
（merged registry を materialize した後、= hover を 1 回叩いた状態）:

| holder | 実測 |
| --- | ---: |
| merged workspace registry | 約60MB |
| stdlib lazy loader（共有 method body / type を含む） | 約33MB |
| workspace state shells + dep graph | 約22MB |
| user RBS + Rails project types | 約15MB |
| per-file snapshots（共有分を除いた正味） | 約5MB |
| floor（全 holder を drop し `mi_collect` 後も残る量） | 約120MB |

floor は 1,200 files の mastodon で約48MB、6,458 files の gitlab で約120MB と workload に比例する。
live data ではなく allocator の retain / fragmentation で、単一 worker（`TYDA_LSP_ANALYSIS_THREADS=1`）
でも約114MB 残るため thread stack や cross-thread heap が主因ではない。RSS をさらに削るなら
snapshot の殻ではなくここが最大の残件。

per-file snapshot の殻は `TypeRegistry` 688B → 360B（snapshot が populate しない 8 つの
collection を `RegistryColdTail` に box 化）、`WorkspaceFileEntry` 1080B → 752B。gitlab/app の
6,458 snapshots で殻の合計は 12.7MB → 8.7MB。プロセス全体の footprint 差は run 間の揺れ
（±4MB）に埋もれる。

## allocator（mimalloc）の扱い

mimalloc は速度面で load-bearing。system allocator に差し替えると gitlab/app の max RSS は
約265MB まで下がるが、elapsed が 4.1–4.6s（約2倍）に悪化する。したがって RSS 削減を
allocator 差し替えで狙わない。

CLI の約135MB（LSP は約120MB）は live data ではなく mimalloc の retain / fragmentation で、
`mi_collect` を呼んでも、全 holder を drop しても残る。この分を削る唯一の手段は
**transient な allocation 量そのものを減らすこと**で、holder の shrink では動かない。

逆に、CPU を削ると peak RSS はわずかに上がることがある。並列 worker が速く進む分だけ
同時に in-flight な per-file データが増えるためで、`[mem]` の live 側（`after-projection`
の deep 内訳）が変わっていなければ回帰ではない。

## per-file analysis の allocation churn（2026-08-26）

samply（gitlab/app、idle thread を除いた active sample 基準）での上位:

| 項目 | before | after |
| --- | ---: | ---: |
| `detect_dsl_libraries_from_source_text`（`str::contains` × 67） | 11.2% | 0% |
| Type の clone / drop / hash / eq / alloc 合計 | 約44% | 約44% |
| SipHash `Hasher::write` | 4.6% | 3.5% |
| active sample 合計 | 7,953 | 7,356 |

対処:

- `detect_realtime_dsl_from_source` は `ActiveSupportConcern` しか使わないので、
  67 marker 全走査をやめて 3 marker だけ見る。
- 全 marker 走査（project 全体 scan / `--dsl` 検出）は 2 byte prefix の bitmap で
  1 pass に畳む。marker 表に無い pattern は `str::contains` に fallback するので、
  表が古くなっても検出結果は変わらない。
- recursion guard / memo の `HashSet` `HashMap`（`visiting`、`memo`、
  `merged_stdlib_classes`、`merged_external_classes`）を Fx hasher へ。いずれも
  membership 専用で iterate しないため出力順に影響しない。
- `merge_external_type_class` の `shared_*` 中間 Vec は borrow を切るためだけの存在なので、
  push 時に clone せず move する。

結果: compact-scan 1.04s → 0.88s（-15%）、user CPU 8.64s → 7.71s（-11%）、
RBS 出力と `--diagnostics` は byte-identical。

## render の profile 内訳（2026-08-26）

`final-resolution+render` バケット（gitlab/app で約0.45–0.49s、全体2.1–2.3sの約21%）を
samply（`write_rbs_from_registry` 実行中のサンプルを、プロセス起動からの累積時刻で
window 抽出）で内訳を見ると、`src/rbs/render.rs` 自身（文字列組み立て・書き込み）は
window 内 active sample の1%未満で、残り99%超は `TypeRegistry::build_output_class_info`
が呼ぶ遅延解決（`resolve_params`・`resolve_deferred_refs_*`・`resolve_method_*_refs`・
`Type` の clone/hash/eq/drop）が占める。一時計測（`Instant` を resolve 呼び出しと
`write_class_body` に個別に仕込み、chunk 内 rayon worker で集計→ revert 済み）でも
resolve : format ≈ 98 : 2（合算 nanos）で一致する。

対処（render.rs のみ、resolution アルゴリズムは変更していない）:

- 各クラスの出力 `Vec<u8>` を `Vec::new()` から `with_capacity`（method / constant / mixin /
  alias 数からの粗い上限見積り）に変更し、`write!` で伸びるたびの再確保を避ける。
- `format_method_sig` / `format_signature` / `format_param` / `format_block_signature` 系を、
  `String` を組み立てて `Vec<String>` に collect → `join` → 上位の `format!` に詰め直す
  多段構成から、`write!`/`write_all` で出力先バッファへ直接書く構成に置き換え（中間
  `String` allocation を除去）。
- `write_rbs_from_classes`（LSP 単一ファイル render）と `write_rbs_from_registry`
  （CLI chunk render）で重複していたクラス本体の書き出しロジックを `write_class_body`
  へ一本化。

結果: gitlab/app・mastodon（full）・mastodon/app とも RBS 出力は byte-identical。
resolve が全体の98%以上を占めるため、render 側の allocation 削減は elapsed に測定可能な
差を出さない（3 runs: 2.14–2.32s、変更前の2.13–2.22sと同じ揺れの範囲内）。

## render の遅延解決の重複除去（2026-08-26）

上節で残件とした resolve 側（`build_output_class_info` が呼ぶ遅延解決）を、出力を変えない
範囲で削った。一時 counter（`build_method_sig_for_receiver` と `resolve_params` に atomic を
仕込み、gitlab/app を 1 回流して revert）で取った内訳:

| 項目 | 実測 |
| --- | ---: |
| `build_method_sig_for_receiver` 呼び出し | 154,750 |
| うち distinct な (class, method, singleton) | 152,480（重複は1.5%のみ） |
| `resolve_params` 呼び出し | 537,795（うち frozen cache miss 65,243） |
| miss のうち `param_infos` が空（即 return） | 64,208（99.8%） |
| `MethodReturnRef` 解決内の `resolve_params` | 393,552 |
| うち解決済み型に `ParamRef` を含まない | 385,988（98.1%） |
| 遅延 ref が残っている return type | 15,882 / 150,659（10.5%、`ReceiverMethodRef` 12,782・`MethodReturnRef` 5,217） |
| `resolve_deferred_refs_for_context` の早期 return | 432,836 / 557,805 |
| `instance` を含む signature | 16 / 154,750 |

分かったこと:

- 同一 (class, method) の再解決は1.5%しかないので、render pass scope の memo は効かない。
  param の frozen cache も 88% は hit し、miss もほぼ「引数なし」の即 return で安い。
- 実際の重複は「解決結果に `ParamRef` が無いのに、その手前で毎回 param を解決していた」
  ことと、「値が変わらないのに Type を deep clone していた」ことの2つ。

対処（解決アルゴリズムと解決順序は変更していない）:

- `MethodReturnRef` 解決で、`resolve_param_refs_from_resolved` が `params` を読むのは
  到達可能な `ParamRef` / `KeywordParamRef` に当たったときだけ。これは
  `type_contains_param_ref` の判定と再帰対象が完全に一致するので、false なら
  `resolve_params` ごと省く（393,552 → 7,564 回）。
- `resolve_instance_type_in_sig` は `instance` を含まない signature に対して
  「同じ値の deep clone を書き戻すだけ」なので、先に走査して含まなければ何もしない。
- `resolve_deferred_refs_for_context` に owned 版を足し、ref を含まない入力を deep clone
  せずそのまま返す（render の return type path で使用）。
- 遅延 ref memo の key `(SharedName, bool, Type)` を `Arc<DeferredKey>` に置き換え、
  hash を構築時に1回だけ計算して保持する。memo と `visiting` の両方を同じ key で引くため、
  従来は 1 node あたり Type の deep hash が3–4回・deep clone が2回走っていた。
- `resolve_method_in_subclasses_refs` の `visited` を `Vec` の線形 `contains` から
  `FxHashSet` へ（membership 専用、探索順と結果順は不変）。widen な継承ツリーで quadratic。
- `lookup_ivar_type_through_ancestors` の祖先ループから `String` 確保を除去（`&str` で辿る）。

`resolve_block_return_refs` の省略は**できない**: `BlockReturnRef` を含まない型に対しても
`Type::Union` を `from_type_vec_preserve_untyped` で正規化し直しており、これが出力に効いている
（省くと gitlab/app で `nil | untyped` が `untyped | nil` になる差分が296行出る）。この関数は
実質 normalizer を兼ねているため、per-method の return type deep clone 1回は残る。

結果（gitlab/app、pre/post を交互に 6 pair、`nice -n 19`）:

| 指標 | before | after |
| --- | ---: | ---: |
| `final-resolution+render` | 0.472–0.541s（平均0.499s） | 0.399–0.459s（平均0.425s） |
| elapsed | 2.16–2.46s（平均2.33s） | 2.14–2.25s（平均2.19s） |
| max RSS | 302–320MB | 295–305MB |

window profile 内の `Type` の deep hash は 7.6% → 2.2%、`resolve_method_in_subclasses_refs` は
2.7% → 上位40圏外。RBS 出力は gitlab/app・mastodon（full）とも byte-identical、
mastodon/app/models の `--diagnostics` も集合一致。

残件: window 内は依然として `Type` の clone / drop / eq / cmp と allocator が約30%で、その多くは
Union の再構築（`from_type_vec_preserve_untyped` の正規化）と memo の値 clone に由来する。
`infer_attr_type_from_initialize` / `resolve_attr_reader_return_type` は render では memo されず
毎回 call site を走査するが、深さ依存の結果を共有 cache に載せることになるため、
byte-identical を担保するには別途 purity の確認が要る。

## 名前空間解決の string / Vec churn（2026-08-27）

samply（gitlab/app、idle thread を除いた active sample 基準）で、`Type` の値操作より上に
名前解決そのもののオーバーヘッドが出ていた。

| 項目 | before | after |
| --- | ---: | ---: |
| `StrSearcher::new` + `TwoWaySearcher`（`"::"` パターン検索） | 4.1% | 0%（`is_contained_in` 0.9% のみ） |
| `format!` 機構（`fmt::write` + `write_str` + `format_inner`）@ compact-scan | 3.7% | 上位圏外 |
| SipHash `Hasher::write` | 2.5% | 1.5% |
| `resolve_scoped_class_ref_borrow`（inclusive） | 4.5–5% | 1.3% |

原因と対処:

- `trim_start_matches("::")` / `rfind("::")` は `&str` pattern なので呼び出しごとに
  `TwoWaySearcher` を構築する。2 byte の needle には無意味なので `NamePath`
  （`trim_scope_prefix` / `rfind_scope_sep` / `contains_scope_sep`）の byte 走査へ置き換える。
  `starts_with` / `strip_prefix` は std 側が直接 memcmp するので触っていない。
- 名前空間結合の `format!("{scope}::{name}")` を exact-capacity な `sym::join_scope` へ。
  `format!` は `String` を無指定容量から伸ばすため、この 1 箇所が per-file analysis 内で
  最大の `RawVec::finish_grow` 発生源だった。
- `resolve_scoped_class_ref_borrow` の scope 走査（enclosing→top）は最初の scope が最長なので、
  ループ全体で 1 本のバッファを `clear` + `push_str` で使い回す（走査あたり 1 allocation）。
- `resolve_method_call_owners_inner_refs` は全 arm が「nested の結果を伝播」か
  「一致したクラスを返す」のどちらかで、必ず要素 1 個の `Vec` しか作らない。
  戻り値を `Option<(&str, bool)>` にして lookup ごとの `Vec` 確保を消す。
- `sym` interner の shard set は membership 専用（`interner_stats` は順序非依存に集計）なので
  SipHash → Fx。shard 選択で既に Fx hash を 1 回払っている。
- `class_or_ancestors_include_module` の BFS は node / mixin ごとに `String` を確保して
  `HashSet<String>` に入れていた。registry 側の値は元から `Arc<str>` なので
  `FxHashSet<SharedName>` にすると refcount 加算だけになる。

結果（gitlab/app、pre/post を交互に 4 pair、`nice -n 19`）:

| 指標 | before | after |
| --- | ---: | ---: |
| compact-scan | 0.923–0.935s（平均0.931s） | 0.772–0.807s（平均0.790s） |
| elapsed | 2.12–2.28s（平均2.19s） | 1.94–2.00s（平均1.97s） |
| max RSS | 305–316MB | 299–311MB |

`[mem]` の deep 内訳（`after-projection` 63.6MB、bodies / call_sites / containers /
param_cache とも）は完全に一致し、live data は変わっていない。減っているのは transient な
allocation 量に対応する allocator の high-water 分だけ。LSP workspace scan は 579ms で回帰なし。
RBS 出力（gitlab/app・mastodon full）と mastodon/app/models の `--diagnostics` は byte-identical。

試して**やめた**もの:

- `merge_external_type_class` の methods ループ前に `data.methods` / `method_index` を
  `reserve` する。lazy stdlib merge は per-file registry に対して走るので、
  over-provision した map の殻が 6,455 files 分積み上がり max RSS が +5MB。
  compact-scan は変わらなかったので差し引きマイナス。
- param cache（`resolve_params`）の `Arc<Vec<Param>>` をそのまま返す共有化。
  cache hit の deep clone は inclusive で 1.4% しかなく、`build_method_sig_for_receiver` が
  `params.retain` と `params[0].param_type` の 2 箇所で条件付きに書き換えてから
  `MethodSig` に move するため、`MethodSig` 側の型変更まで巻き込む。効果に対して面積が大きい。
- rayon worker ごとの scratch 使い回し（per-file の `TypeRegistry` / buffer 再利用）。
  compact-scan 内の allocator sample は 6.3%、`retain_file_facts` は inclusive 2.0% しかなく、
  「全 buffer が files 間で確実に clear される」ことを保証する面積に見合わない。

## 遅延解決クラスタの実測比重（2026-08-27）

`samply`（gitlab/app、`CARGO_PROFILE_RELEASE_DEBUG=1`、idle stack を除いた active sample
基準）で、遅延解決の関数群を含む stack だけを窓として切り出した内訳。**この塊は active
sample の 13.2%** で、以前の「inclusive 28% / 26%」は idle と I/O を含んだ見積りだった。

| 窓内の self time | 割合（窓内 / active 全体） |
| --- | ---: |
| `Type` の clone / drop / eq / Ord / hash | 24.9% / 3.3% |
| memcmp + memmove | 9.2% / 1.2% |
| allocator（mi_malloc / mi_free / retire / `finish_grow`） | 8.9% / 1.2% |
| `MethodIndex::get` | 4.1% / 0.5% |
| union 正規化（`collect_union_parts` / `subsume_literals` / `Type::cmp`） | 6.6% / 0.9% |

つまりクラスタ内の `Type` churn を**全部消しても上限は active の 3.3%**で、
`final-resolution+render` の 0.4s に対して 0.05–0.08s が天井。実際に効いたのは
`MethodIndex::get` の 1 件だけだった。

一時カウンタで採った呼び出し実績（gitlab/app）:

| 対象 | 呼び出し | memo key 生成 | memo hit |
| --- | ---: | ---: | ---: |
| `resolve_call_site_type_from_caller_context` | 5,804,904 | 2,384,502 | 5,347（0.22%） |
| `resolve_deferred_refs_depth` | 1,925,576 | 1,390,106 | 112,829（8.1%） |
| `resolve_deferred_refs_for_context`（hop） | 439,747 | 53,081 | 30,272（57%） |
| `MethodIndex::get`（frozen） | 2,166,688 | - | 12,127,907 probe（5.6/lookup） |

入れた対処:

- caller-context walk が書き換えるのは `ParamRef` / `KeywordParamRef` だけで、他の node は
  子から再構築しても入力と一致する。例外は `Type::Union` の
  `from_type_vec_preserve_untyped` による再正規化（これは出力に効く: 全 5.80M 呼び出しのうち
  795 件が「param ref 無しなのに値が変わる」）。両方を含まない subtree を先読みで弾く
  （3,724,918 / 5,804,904 = 64%）。
- `MethodIndex` の frozen entry に名前先頭 8 byte を big-endian u64 で持たせる。識別子に NUL は
  無いので u64 順は `str` 順と一致し、同値は full name へ落ちるので**配列順と iteration 順は不変**。
  1 lookup 5.6 probe が interned 名前を deref しなくなる。24B/entry（+8B、gitlab で live +1.2MB、
  max RSS の run 間揺れに埋もれる）。

結果（gitlab/app、pre/post を交互に 6 pair、順序も交互、`nice -n 19`）:

| 指標 | before | after |
| --- | ---: | ---: |
| `final-resolution+render` | 0.367–0.440s（平均0.407s） | 0.355–0.398s（平均0.372s） |
| `merge` | 0.443–0.473s（平均0.455s） | 0.433–0.453s（平均0.444s） |
| elapsed | 1.815–2.073s（中央1.851s） | 1.797–1.912s（中央1.836s） |
| max RSS | 277–303MB | 283–297MB |

`MethodIndex::get` の profile sample は 418 → 350（うち memcmp 184 → 66）。
LSP workspace scan は gitlab/app で 442–500ms（3 runs）で回帰なし。RBS 出力（gitlab/app・
mastodon full）と mastodon/app/models の `--diagnostics` は byte-identical。

試して**やめた**もの:

- caller-context walk の識別スキップ**単体**では wall clock が動かない。省けるのは memo key の
  deep clone と hash だが、その memo の hit 率が 0.22% しかなく元から仕事量が小さい。
  measurable な差になったのは `MethodIndex` と合わせてから。
- 遅延 ref memo から container node（`Union` / `Hash` / `Record` …）の key を落とす。
  key 生成 1,390,106 件のうち container は 342,911 件で hit は 11,001 件しかないが、
  `visiting` による cycle cut が container で 3,463 件実際に起きているので、
  落とすと cut 位置が変わり出力が変わりうる。
- 祖先探索 `resolve_method_call_owners_inner_refs` の `seen` 線形走査に fingerprint（末尾 8 byte
  + 長さ）の fast-reject を足す。memcmp sample は 157 → 132 に減るが、fingerprint 計算と
  24B 化した要素のぶんで関数の inclusive sample が 998 → 1,185 に増えて差し引きマイナス。
  この関数の memcmp は `seen.contains` 由来が主ではない。
- caller-context walk の `Union` arm に「子が変わらなければ再正規化しない」を入れる。
  上記 795 件がそこで変わっているので byte-identical を破る。

残件（クラスタ外の実測上位）:

- `open()` / `read()` の syscall が active sample の 20–25%。6,455 files の 1 file 1 open で、
  algorithmic な重複ではない（macOS 側のコスト）。
- `resolve_method_call_owners_inner_refs` が inclusive で active の 7.9%。ただし呼び出し元の
  ほとんどは compact-scan（`InferenceEngine::infer_node_type` 系）で、そこは per-file registry が
  mutable なため `owner_lookup_cache` / `first_owner_cache` が無効。遅延解決経由は 4% 程度。
- `resolve_params_inner` は caller-context 経路で param 列を毎回作り直して 1 個だけ使う。
  ただし cache が効く経路（`PARAM_TABLE_MODE`）の deep clone は窓内 6 sample しかなく、
  実費は `visiting` 依存で cache できない再計算側にある。

## 記録ルール

- 現在の基準を上書きし、履歴表を増やさない。
- 速度は原則 3 runs の平均、揺れが大きい指標は範囲も残す。
- memory は max RSS を残し、測定不能なら `-` とする。
- benchmark subject と build mode を必ず表に書く。
- cache、workspace state、registry、inference、LSP の変更では関係する基準を再計測する。
- byte-identical な RBS 出力と diagnostics の意味を性能改善の必須条件にする。

大きな回帰を見つけたら、原因の切り分けは benchmark output と git の一時ブランチで行い、
この文書には再現コマンドと現在の基準だけを残す。
