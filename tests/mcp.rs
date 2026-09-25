use std::time::Duration;

use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::json;
use tokio::process::Command;

#[tokio::test]
async fn mcp_stdio_server_starts_and_infers_type() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let transport = TokioChildProcess::new(Command::new(env!("CARGO_BIN_EXE_tyda")).configure(
            |command| {
                command.arg("mcp");
            },
        ))
        .expect("start Tyda MCP server process");
        let client = ().serve(transport).await.expect("initialize MCP client");

        let tools = client.list_all_tools().await.expect("list MCP tools");
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "infer_type_at_position"),
            "MCP server should advertise infer_type_at_position"
        );

        let result = client
            .call_tool(
                CallToolRequestParams::new("infer_type_at_position").with_arguments(
                    json!({
                        "source": "name = \"Tyda\"\nname",
                        "line": 1,
                        "character": 0,
                    })
                    .as_object()
                    .expect("tool arguments are a JSON object")
                    .clone(),
                ),
            )
            .await
            .expect("call inference tool");

        assert_ne!(result.is_error, Some(true), "tool call should succeed");
        let structured = result
            .structured_content
            .expect("tool result should contain structured data");
        assert_eq!(structured["found"], true);
        assert_eq!(structured["inferred_type"], "\"Tyda\"");

        client.cancel().await.expect("shut down MCP server process");
    })
    .await
    .expect("MCP server did not complete the protocol exchange within 30 seconds");
}
