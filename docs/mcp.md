# MCP server

Tyda gem はローカル stdio MCP server を含む。AI agent は `infer_type_at_position` を呼び、渡した Ruby source の指定位置にある式の推論型を取得できる。

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
