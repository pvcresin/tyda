use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use rmcp::{
    ErrorData as McpError, ServiceExt, handler::server::wrapper::Parameters, model::CallToolResult,
    tool, tool_router, transport::io::stdio,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::analysis::{AnalysisOptions, HoverResult, hover_at_with_analysis_options};
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::rbs::workspace::default_vendor_rbs_root;

const MAX_SOURCE_BYTES: usize = 262_144;

#[derive(Clone)]
struct TydaMcpServer {
    stdlib_loader: Arc<LazyRbsLoader>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct InferTypeAtPositionParams {
    #[schemars(description = "Ruby source or a focused snippet containing the expression.")]
    source: String,
    #[schemars(description = "Zero-based line number, following the LSP position convention.")]
    line: u32,
    #[schemars(
        description = "Zero-based UTF-16 code-unit column, following the LSP position convention."
    )]
    character: u32,
}

impl TydaMcpServer {
    fn new() -> Self {
        let core_rbs = default_vendor_rbs_root().join("core");
        Self {
            stdlib_loader: Arc::new(LazyRbsLoader::new(core_rbs)),
        }
    }
}

#[tool_router(server_handler)]
impl TydaMcpServer {
    #[tool(
        description = "Infer the type at a position in the provided Ruby source. Pass the complete relevant source or a focused snippet. Positions are zero-based and character uses UTF-16 code units, matching LSP. This tool analyzes only the supplied source with Tyda's bundled standard-library RBS; it does not read or scan workspace files."
    )]
    async fn infer_type_at_position(
        &self,
        Parameters(params): Parameters<InferTypeAtPositionParams>,
    ) -> Result<CallToolResult, McpError> {
        let loader = Arc::clone(&self.stdlib_loader);
        let response = tokio::task::spawn_blocking(move || build_inference_result(params, &loader))
            .await
            .map_err(|error| {
                McpError::internal_error(format!("Tyda inference task failed: {error}"), None)
            })?;

        Ok(match response {
            Ok(result) => CallToolResult::structured(result),
            Err(error) => CallToolResult::structured_error(error),
        })
    }
}

fn build_inference_result(
    params: InferTypeAtPositionParams,
    stdlib_loader: &LazyRbsLoader,
) -> Result<Value, Value> {
    if params.source.len() > MAX_SOURCE_BYTES {
        return Err(json!({
            "code": "source_too_large",
            "message": format!("Source exceeds the {MAX_SOURCE_BYTES}-byte limit; pass a focused snippet."),
        }));
    }

    let Some((line, byte_column)) = source_position(&params.source, params.line, params.character)
    else {
        return Err(json!({
            "code": "invalid_position",
            "message": "The line and UTF-16 character position must identify a valid position in the source.",
        }));
    };

    let started_at = Instant::now();
    let hover = hover_at_with_analysis_options(
        &params.source,
        None,
        stdlib_loader,
        "mcp_input.rb",
        line,
        byte_column,
        AnalysisOptions::default(),
    );
    let analysis_time_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);

    Ok(match hover {
        Some(hover) => hover_result(hover, analysis_time_ms),
        None => json!({
            "found": false,
            "message": "Tyda could not infer a type at this position.",
            "analysis_time_ms": analysis_time_ms,
        }),
    })
}

fn hover_result(hover: HoverResult, analysis_time_ms: u64) -> Value {
    let type_parameters = hover
        .type_params
        .into_iter()
        .map(|(name, ty)| json!({ "name": name, "type": ty.to_string() }))
        .collect::<Vec<_>>();

    json!({
        "found": true,
        "name": hover.name,
        "inferred_type": hover.ty.to_string(),
        "display_rbs": hover.display_rbs,
        "type_parameters": type_parameters,
        "unresolved_method": hover.unresolved_method,
        "analysis_time_ms": analysis_time_ms,
    })
}

fn source_position(source: &str, line: u32, character: u32) -> Option<(usize, usize)> {
    let line_index = usize::try_from(line).ok()?;
    let target_utf16_column = usize::try_from(character).ok()?;
    let one_based_line = line_index.checked_add(1)?;
    let source_line = source.split('\n').nth(line_index)?;
    let source_line = source_line.strip_suffix('\r').unwrap_or(source_line);

    let mut utf16_column = 0_usize;
    for (byte_column, scalar) in source_line.char_indices() {
        if utf16_column == target_utf16_column {
            return Some((one_based_line, byte_column));
        }

        utf16_column = utf16_column.checked_add(scalar.len_utf16())?;
        if target_utf16_column < utf16_column {
            return None;
        }
    }

    (utf16_column == target_utf16_column).then_some((one_based_line, source_line.len()))
}

pub async fn serve() -> Result<(), Box<dyn Error + Send + Sync>> {
    let service = TydaMcpServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_lsp_utf16_position_to_utf8_column() {
        assert_eq!(source_position("name = '猫'\n", 0, 9), Some((1, 11)));
        assert_eq!(source_position("x = '🍣'\n", 0, 6), None);
        assert_eq!(source_position("x = '🍣'\n", 0, 7), Some((1, 9)));
    }

    #[test]
    fn returns_inferred_type_for_supplied_source() {
        let stdlib_loader = LazyRbsLoader::new(default_vendor_rbs_root().join("core"));
        let result = build_inference_result(
            InferTypeAtPositionParams {
                source: "name = \"Tyda\"\nname".to_owned(),
                line: 1,
                character: 0,
            },
            &stdlib_loader,
        )
        .expect("inference result");

        assert_eq!(result["found"].as_bool(), Some(true));
        assert_eq!(result["inferred_type"].as_str(), Some("\"Tyda\""));
    }
}
