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

use crate::analysis::{
    AnalysisOptions, AnalysisQueryLocation, HoverResult, TypeExplanationLimits,
    explain_type_at_with_analysis_options, hover_at_with_analysis_options,
};
use crate::inference::hover::TypeExplanation;
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::rbs::workspace::default_vendor_rbs_root;

const MAX_SOURCE_BYTES: usize = 262_144;
const DEFAULT_EXPLANATION_DEPTH: u8 = 3;
const MAX_EXPLANATION_DEPTH: u8 = 5;
const MAX_EXPLANATION_STEPS: usize = 32;

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

#[derive(Debug, Deserialize, JsonSchema)]
struct ExplainTypeAtPositionParams {
    #[schemars(description = "Ruby source or a focused snippet containing the expression.")]
    source: String,
    #[schemars(description = "Zero-based line number, following the LSP position convention.")]
    line: u32,
    #[schemars(
        description = "Zero-based UTF-16 code-unit column, following the LSP position convention."
    )]
    character: u32,
    #[schemars(
        description = "Maximum method-body call hops to include; defaults to 3 and is capped at 5."
    )]
    max_depth: Option<u8>,
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

    #[tool(
        description = "Explain the inferred type at a position in the provided Ruby source. Returns method declaration origins, inferred body return types, and matching call sites found in that source. Positions are zero-based and character uses UTF-16 code units, matching LSP. This tool analyzes only the supplied source with Tyda's bundled standard-library RBS; it does not read or scan workspace files."
    )]
    async fn explain_type_at_position(
        &self,
        Parameters(params): Parameters<ExplainTypeAtPositionParams>,
    ) -> Result<CallToolResult, McpError> {
        let loader = Arc::clone(&self.stdlib_loader);
        let response =
            tokio::task::spawn_blocking(move || build_explanation_result(params, &loader))
                .await
                .map_err(|error| {
                    McpError::internal_error(format!("Tyda explanation task failed: {error}"), None)
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

fn build_explanation_result(
    params: ExplainTypeAtPositionParams,
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

    let max_depth = params
        .max_depth
        .unwrap_or(DEFAULT_EXPLANATION_DEPTH)
        .min(MAX_EXPLANATION_DEPTH);
    let started_at = Instant::now();
    let explanation = explain_type_at_with_analysis_options(
        &params.source,
        None,
        stdlib_loader,
        AnalysisQueryLocation {
            file_path: "mcp_input.rb",
            line,
            byte_column,
        },
        AnalysisOptions::default(),
        TypeExplanationLimits {
            max_depth: usize::from(max_depth),
            max_steps: MAX_EXPLANATION_STEPS,
        },
    );
    let analysis_time_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);

    Ok(match explanation {
        Some(explanation) => {
            explanation_result(explanation, &params.source, analysis_time_ms, max_depth)
        }
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

fn explanation_result(
    explanation: TypeExplanation,
    source: &str,
    analysis_time_ms: u64,
    max_depth: u8,
) -> Value {
    let mut result = hover_result(explanation.hover, analysis_time_ms);
    let steps: Vec<Value> = explanation
        .steps
        .iter()
        .map(|step| {
            json!({
                "depth": step.depth,
                "parent": step.parent,
                "relation": step.relation,
                "method": step.method.as_ref().map(|method| json!({
                    "owner": method.owner,
                    "name": method.name,
                    "definition_name": method.definition_name,
                    "is_singleton": method.is_singleton,
                    "signature": method.signature,
                    "origins": method.origins,
                    "declaration": {
                        "path": method.declaration_path,
                        "line": method.declaration_location.map(|location| location.line),
                        "column": method.declaration_location.map(|location| location.column),
                    },
                })),
                "call_site": step.call_site.as_ref().map(|site| explanation_call_site_json(site, source)),
                "callers": step.callers.iter().map(|site| explanation_call_site_json(site, source)).collect::<Vec<_>>(),
                "callers_truncated": step.callers_truncated,
                "body_returns_truncated": step.body_returns_truncated,
                "body_returns": step.body_returns.iter().map(|body| json!({
                    "owner": body.owner,
                    "method_name": body.method_name,
                    "source": source_span_json(source, body.start, body.end),
                    "inferred_type": body.inferred_type.to_string(),
                    "declared_types": body.declared_types.as_ref().map(|types| types.iter().map(ToString::to_string).collect::<Vec<_>>()),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let message = explanation
        .steps
        .is_empty()
        .then_some("No method call or method definition trace is available at this position.");
    if let Some(object) = result.as_object_mut() {
        object.insert(
            "explanation".to_string(),
            json!({
                "scope": "provided_source_and_bundled_stdlib_rbs",
                "max_depth": max_depth,
                "truncated": explanation.truncated,
                "message": message,
                "steps": steps,
                "limitations": [
                    "Call sites are limited to the provided source.",
                    "Project RBS and RBI files and unsaved editor buffers are not loaded.",
                    "Method body call links show expressions inside the body; they do not prove that every branch contributes to the return type.",
                ],
            }),
        );
    }
    result
}

fn explanation_call_site_json(
    site: &crate::inference::hover::TypeExplanationCallSite,
    source: &str,
) -> Value {
    json!({
        "method_name": site.method_name,
        "source": source_span_json(source, site.start, site.end),
        "receiver_type": site.receiver_type.to_string(),
        "inferred_type": site.inferred_type.as_ref().map(ToString::to_string),
        "arguments": site.arguments.iter().map(|argument| json!({
            "name": argument.keyword,
            "position": argument.positional_index,
            "source": source_span_json(source, argument.start, argument.end),
            "inferred_type": argument.inferred_type.to_string(),
        })).collect::<Vec<_>>(),
    })
}

fn source_span_json(source: &str, start: usize, end: usize) -> Value {
    json!({
        "start": source_position_for_offset(source, start),
        "end": source_position_for_offset(source, end),
        "text": source.get(start..end),
    })
}

fn source_position_for_offset(source: &str, offset: usize) -> Option<Value> {
    let prefix = source.get(..offset)?;
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = match prefix.rfind('\n') {
        Some(index) => index.checked_add(1)?,
        None => 0,
    };
    let character = prefix.get(line_start..)?.encode_utf16().count();
    Some(json!({ "line": line, "character": character }))
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

    #[test]
    fn explains_method_return_and_call_sites() {
        let stdlib_loader = LazyRbsLoader::new(default_vendor_rbs_root().join("core"));
        let result = build_explanation_result(
            ExplainTypeAtPositionParams {
                source: r#"class Example
  def identify(value)
    value
  end
end
Example.new.identify("first")
Example.new.identify(1)
"#
                .to_owned(),
                line: 5,
                character: 12,
                max_depth: Some(3),
            },
            &stdlib_loader,
        )
        .expect("type explanation");

        assert_eq!(result["found"].as_bool(), Some(true));
        let steps = result["explanation"]["steps"]
            .as_array()
            .expect("explanation steps");
        let root = steps.first().expect("queried method step");
        assert_eq!(root["method"]["name"].as_str(), Some("identify"));
        assert_eq!(root["method"]["owner"].as_str(), Some("Example"));
        assert_eq!(root["callers"].as_array().map(Vec::len), Some(2));
        assert_eq!(root["body_returns"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            root["call_site"]["arguments"][0]["inferred_type"].as_str(),
            Some("\"first\"")
        );
    }

    #[test]
    fn identifies_inline_sig_and_stdlib_rbs_origins() {
        let stdlib_loader = LazyRbsLoader::new(default_vendor_rbs_root().join("core"));
        let source = r#"class Example
  #: () -> String
  def label
    "label"
  end

  extend T::Sig
  sig { returns(Integer) }
  def count
    1
  end
end
Example.new.label
Example.new.count
"#;
        let inline = build_explanation_result(
            ExplainTypeAtPositionParams {
                source: source.to_owned(),
                line: 12,
                character: 12,
                max_depth: None,
            },
            &stdlib_loader,
        )
        .expect("inline RBS explanation");
        assert_eq!(inline["found"], true);
        assert!(
            inline["explanation"]["steps"][0]["method"]["origins"]
                .as_array()
                .is_some_and(|origins| origins.iter().any(|origin| origin == "rbs_comment"))
        );

        let sorbet = build_explanation_result(
            ExplainTypeAtPositionParams {
                source: source.to_owned(),
                line: 8,
                character: 6,
                max_depth: None,
            },
            &stdlib_loader,
        )
        .expect("Sorbet sig explanation");
        assert_eq!(sorbet["found"], true);
        assert!(
            sorbet["explanation"]["steps"][0]["method"]["origins"]
                .as_array()
                .is_some_and(|origins| origins.iter().any(|origin| origin == "sorbet_sig"))
        );

        let stdlib = build_explanation_result(
            ExplainTypeAtPositionParams {
                source: "name = \"Tyda\"\nname.length".to_owned(),
                line: 1,
                character: 5,
                max_depth: None,
            },
            &stdlib_loader,
        )
        .expect("stdlib RBS explanation");
        assert_eq!(stdlib["found"], true);
        let origins = &stdlib["explanation"]["steps"][0]["method"]["origins"];
        assert!(
            origins
                .as_array()
                .is_some_and(|origins| origins.iter().any(|origin| origin == "rbs_file")),
            "stdlib method origins: {origins}; explanation: {stdlib}"
        );
    }

    #[test]
    fn bounds_method_body_trace_by_requested_depth() {
        let stdlib_loader = LazyRbsLoader::new(default_vendor_rbs_root().join("core"));
        let result = build_explanation_result(
            ExplainTypeAtPositionParams {
                source: r#"class Example
  def first
    self.second
  end
  def second
    self.third
  end
  def third
    "done"
  end
end
Example.new.first
"#
                .to_owned(),
                line: 11,
                character: 12,
                max_depth: Some(1),
            },
            &stdlib_loader,
        )
        .expect("bounded type explanation");

        let steps = result["explanation"]["steps"]
            .as_array()
            .expect("explanation steps");
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1]["method"]["name"], "second");
        assert_eq!(steps[1]["parent"], 0);
        assert_eq!(steps[1]["relation"], "method_body_call");
        assert_eq!(result["explanation"]["truncated"], true);
    }
}
