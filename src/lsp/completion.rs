use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionTextEdit, TextEdit};

use crate::rbs::display::{format_method_sig_for_lens_with_names, user_facing_type};
use crate::registry::{
    ConstantCompletionCandidate, ConstantCompletionKind, MethodCompletionCandidate,
};

use super::source_support::ConstantPathCompletionContext;

pub(super) fn method_completion_items(
    candidates: Vec<MethodCompletionCandidate>,
    replace_range: tower_lsp::lsp_types::Range,
    output_parameter_names: bool,
) -> Vec<CompletionItem> {
    candidates
        .into_iter()
        .enumerate()
        .map(|(idx, candidate)| {
            let label = candidate.name.clone();
            CompletionItem {
                label: label.clone(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(method_completion_detail(&candidate, output_parameter_names)),
                sort_text: Some(format!("{idx:04}_{label}")),
                filter_text: Some(label.clone()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text: label,
                })),
                ..Default::default()
            }
        })
        .collect()
}

fn method_completion_detail(
    candidate: &MethodCompletionCandidate,
    output_parameter_names: bool,
) -> String {
    let separator = if candidate.is_singleton { "." } else { "#" };
    format!(
        "{}{}{} : {}",
        candidate.owner_class,
        separator,
        candidate.name,
        format_method_sig_for_lens_with_names(&candidate.sig, output_parameter_names)
    )
}

pub(super) fn constant_completion_items(
    candidates: Vec<ConstantCompletionCandidate>,
    context: &ConstantPathCompletionContext,
) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = candidates
        .into_iter()
        .map(|candidate| constant_completion_item(candidate, context.replace_range))
        .collect();
    for (idx, item) in items.iter_mut().enumerate() {
        item.sort_text = Some(format!("{idx:04}_{}", item.label));
        item.filter_text = Some(item.label.clone());
    }
    items
}

fn constant_completion_item(
    candidate: ConstantCompletionCandidate,
    replace_range: tower_lsp::lsp_types::Range,
) -> CompletionItem {
    let label = candidate.name;
    let (kind, detail) = match candidate.kind {
        ConstantCompletionKind::Class => (
            CompletionItemKind::CLASS,
            format!("class {}", candidate.full_name),
        ),
        ConstantCompletionKind::Module => (
            CompletionItemKind::MODULE,
            format!("module {}", candidate.full_name),
        ),
        ConstantCompletionKind::Constant => {
            let detail = candidate
                .const_type
                .as_ref()
                .map(|ty| format!("{} : {}", candidate.full_name, user_facing_type(ty)))
                .unwrap_or(candidate.full_name);
            (CompletionItemKind::CONSTANT, detail)
        }
    };
    CompletionItem {
        label: label.clone(),
        kind: Some(kind),
        detail: Some(detail),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: replace_range,
            new_text: label,
        })),
        ..Default::default()
    }
}
