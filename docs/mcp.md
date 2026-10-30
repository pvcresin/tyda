# MCP server

Tyda gem はローカル stdio MCP server を含む。AI agent は `infer_type_at_position` で型を取得し、`explain_type_at_position` で推論に関係するコードと宣言を確認できる。

この server は渡された source と gem に同梱された stdlib RBS だけを解析する。workspace の走査や project 固有の `.rbs` / `.rbi` の読み込みは行わない。agent は必要な Ruby source を読み取り、推論したい箇所を含む内容を tool に渡す。

## インストール

| 管理方法 | インストール | MCP server command |
|---|---|---|
| gem | `gem install tyda` | `tyda mcp` |
| Bundler | `bundle add tyda --group development` | `bundle exec tyda mcp` |

設定例は gem を直接起動する形。Bundler では `command` を `bundle`、`args` を `["exec", "tyda", "mcp"]` にする。CLI 登録では `-- bundle exec tyda mcp` を指定し、作業ディレクトリは Gemfile があるプロジェクトにする。

## Client 設定

### Codex

Codex CLI に登録する。

```sh
codex mcp add tyda -- tyda mcp
codex mcp list
```

`tyda` が `PATH` にない場合は、`command` に gem の実行ファイルの絶対パスを設定する。

```toml
[mcp_servers.tyda]
command = "/absolute/path/to/tyda"
args = ["mcp"]
```

### VS Code / GitHub Copilot

workspace の `.vscode/mcp.json` に stdio server を設定する。

```json
{
  "servers": {
    "tyda": {
      "type": "stdio",
      "command": "tyda",
      "args": ["mcp"]
    }
  }
}
```

`command` は VS Code のプロセスから実行できる必要がある。`PATH` が異なる場合は絶対パスに置き換える。

### Claude Code

Claude Code CLI に stdio server を登録する。

```sh
claude mcp add --transport stdio tyda -- tyda mcp
claude mcp list
```

リポジトリで設定を共有する場合は `--scope project` を追加する。この設定は `.mcp.json` に保存され、Claude Code で利用を承認すると接続できる。

```sh
claude mcp add --transport stdio --scope project tyda -- tyda mcp
```

`tyda` が `PATH` にない場合は、`tyda mcp` を実行ファイルの絶対パスに置き換える。

## Tool: `infer_type_at_position`

入力:

| Field | 型 | 説明 |
|---|---|---|
| `source` | `string` | 推論対象の Ruby source。最大 262,144 bytes |
| `line` | `integer` | 0 始まりの行番号 |
| `character` | `integer` | 0 始まりの UTF-16 code unit 列。LSP の position と同じ |

例:

```json
{
  "source": "name = \"Tyda\"\nname",
  "line": 1,
  "character": 0
}
```

結果は `found` と推論型 `inferred_type` を中心とした JSON object で返る。`display_rbs` があれば型の RBS 表記として参照できる。型が見つからない場合は `found: false` になる。source 上限や位置が不正な場合は `code` と `message` を含む tool error を返す。

```json
{
  "found": true,
  "name": "name",
  "inferred_type": "\"Tyda\"",
  "display_rbs": null,
  "type_parameters": [],
  "unresolved_method": null,
  "analysis_time_ms": 2
}
```

agent は対象ファイルから十分な文脈を含む source を渡し、式の中の位置を指定する。複数ファイルの定義や project の独自 RBS に依存する推論では、それらを含む source snippet を組み立てるか、通常の CLI による全体解析を使う。

## Tool: `explain_type_at_position`

型を調べる位置は `infer_type_at_position` と同じ形式で指定する。`max_depth` はメソッド本体内の呼び出しを何段辿るかを指定し、省略時は `3`、最大は `5`。結果は通常の型情報に加えて、次を `explanation.steps` に返す。

- メソッドの宣言元: Ruby 推論、inline RBS comment、Sorbet `sig`、`.rbs`、synthetic DSL の識別情報、宣言位置、取得できたメソッド署名
- 入力source内の同じメソッドの呼び出し元と各引数の推論型
- source内で解析できたメソッド本体の戻り値と、本体内の呼び出し
- `depth`、`parent`、`relation` による呼び出し経路

`max_depth` は最大 5、出力step数は最大 32、各メソッドの呼び出し元は最大 16 件、本体戻り値の証拠は最大 8 件に制限する。上限に達した結果では `explanation.truncated`、`callers_truncated`、または `body_returns_truncated` が `true` になる。出力内のsource spanはLSPと同じ0始まりの行番号・UTF-16 code unit列。宣言位置は1始まりの行番号と0始まりのUTF-8 byte列。

説明対象は現在メソッド呼び出しとメソッド定義に限る。その他の式では `explanation.steps` が空になり、`explanation.message` に理由が入る。このtoolも渡されたsourceと同梱stdlib RBSだけを解析する。workspaceの他ファイル、project固有のRBS/RBI、未保存のeditor bufferは読まないため、呼び出し元の範囲も入力source内に限られる。メソッド本体内の呼び出しは本体に存在する式の証拠であり、すべての分岐が戻り値に寄与することを示すものではない。
