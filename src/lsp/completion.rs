use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Range, TextEdit,
};

use crate::completion::{CompletionEntry, CompletionEntryKind};

pub(super) fn completion_items(
    entries: Vec<CompletionEntry>,
    replace_range: Range,
) -> Vec<CompletionItem> {
    entries
        .into_iter()
        .enumerate()
        .map(|(idx, entry)| CompletionItem {
            label: entry.label.clone(),
            kind: Some(completion_item_kind(entry.kind)),
            detail: Some(entry.detail),
            sort_text: Some(format!("{idx:04}_{}", entry.label)),
            filter_text: Some(entry.label.clone()),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: replace_range,
                new_text: entry.label,
            })),
            ..Default::default()
        })
        .collect()
}

fn completion_item_kind(kind: CompletionEntryKind) -> CompletionItemKind {
    match kind {
        CompletionEntryKind::Method => CompletionItemKind::METHOD,
        CompletionEntryKind::Class => CompletionItemKind::CLASS,
        CompletionEntryKind::Module => CompletionItemKind::MODULE,
        CompletionEntryKind::Constant => CompletionItemKind::CONSTANT,
    }
}
