use std::path::PathBuf;

use tyda::analysis::{PlaygroundResult, playground_analyze};
use tyda::rbs::stdlib_loader::LazyRbsLoader;

#[derive(Clone, Copy)]
struct ExpectedHover {
    token: &'static str,
    occurrence: usize,
    ready_after: &'static str,
    name: &'static str,
    display: &'static str,
}

fn loader() -> LazyRbsLoader {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
    LazyRbsLoader::new(core_dir)
}

fn offset_of(source: &str, token: &str, occurrence: usize) -> usize {
    source
        .match_indices(token)
        .nth(occurrence)
        .map(|(offset, _)| offset)
        .unwrap_or_else(|| panic!("missing occurrence {occurrence} of {token:?}"))
}

fn line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1;
    let mut column = 0;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn offset_at(source: &str, line: u32, column: u32) -> Option<usize> {
    let mut offset = 0;
    for (index, current) in source.split('\n').enumerate() {
        if index + 1 == line as usize {
            let byte_column = current
                .char_indices()
                .nth(column as usize)
                .map(|(index, _)| index)
                .unwrap_or_else(|| current.len());
            return (column as usize <= current.chars().count()).then_some(offset + byte_column);
        }
        offset += current.len() + 1;
    }
    None
}

fn assert_valid_hover_ranges(source: &str, result: &PlaygroundResult) {
    for hover in &result.hovers {
        assert!(!hover.display.is_empty(), "empty hover display: {hover:?}");
        let start = offset_at(source, hover.line, hover.column)
            .unwrap_or_else(|| panic!("hover starts outside source: {hover:?}"));
        let end = offset_at(source, hover.end_line, hover.end_column)
            .unwrap_or_else(|| panic!("hover ends outside source: {hover:?}"));
        assert!(start < end, "empty hover range: {hover:?}");
        assert!(end <= source.len(), "hover ends outside source: {hover:?}");
    }
}

fn assert_incremental_hovers(source: &str, expectations: &[ExpectedHover]) {
    let loader = loader();
    let offsets: Vec<usize> = source
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(source.len()))
        .collect();

    let resolved: Vec<(usize, usize, ExpectedHover)> = expectations
        .iter()
        .copied()
        .map(|expected| {
            let start = offset_of(source, expected.token, expected.occurrence);
            let end = start + expected.token.len();
            let ready_after = source
                .find(expected.ready_after)
                .unwrap_or_else(|| panic!("missing readiness marker {:?}", expected.ready_after))
                + expected.ready_after.len();
            (start, end.max(ready_after), expected)
        })
        .collect();

    for prefix_end in offsets {
        let prefix = &source[..prefix_end];
        let result = playground_analyze(prefix, "", &loader, "incremental.rb");
        assert_valid_hover_ranges(prefix, &result);

        for &(start, ready_end, expected) in &resolved {
            if prefix_end < ready_end {
                continue;
            }
            let (line, column) = line_col(prefix, start);
            let hover = result
                .hovers
                .iter()
                .find(|hover| hover.line == line && hover.column == column)
                .unwrap_or_else(|| {
                    panic!(
                        "missing hover for {} at prefix {prefix_end}: {prefix:?}",
                        expected.token
                    )
                });
            assert_eq!(hover.name, expected.name, "name at prefix {prefix_end}");
            assert_eq!(
                hover.display, expected.display,
                "display for {} at prefix {prefix_end}: {prefix:?}",
                expected.token
            );
        }
    }
}

#[test]
fn annotated_literal_hovers_remain_correct_after_each_keystroke() {
    let source = concat!(
        "class User\n",
        "  #: (\"test\") -> void\n",
        "  def initialize(name)\n",
        "    @name = name\n",
        "  end\n",
        "\n",
        "  def name = @name\n",
        "\n",
        "  def greeting = \"hello, #{@name}\"\n",
        "end\n",
    );
    assert_incremental_hovers(
        source,
        &[
            ExpectedHover {
                token: "User",
                occurrence: 0,
                ready_after: "class User",
                name: "User",
                display: "[Tyda] singleton(User)",
            },
            ExpectedHover {
                token: "name",
                occurrence: 0,
                ready_after: "    @name = name",
                name: "name",
                display: "[Tyda] \"test\"",
            },
            ExpectedHover {
                token: "@name",
                occurrence: 0,
                ready_after: "    @name = name",
                name: "@name",
                display: "[Tyda] \"test\"",
            },
            ExpectedHover {
                token: "name",
                occurrence: 3,
                ready_after: "  def name = @name",
                name: "name",
                display: "[Tyda] -> \"test\"",
            },
            ExpectedHover {
                token: "greeting",
                occurrence: 0,
                ready_after: "  def greeting = \"hello, #{@name}\"",
                name: "greeting",
                display: "[Tyda] -> \"hello, test\"",
            },
        ],
    );
}

#[test]
fn class_constant_and_local_hovers_remain_correct_after_each_keystroke() {
    let source = concat!(
        "module Billing\n",
        "  TAX_RATE = 0.1\n",
        "\n",
        "  class Invoice\n",
        "    #: (Integer total) -> void\n",
        "    def initialize(total)\n",
        "      @total = total\n",
        "    end\n",
        "\n",
        "    def amount\n",
        "      local = @total\n",
        "      local\n",
        "    end\n",
        "  end\n",
        "end\n",
    );
    assert_incremental_hovers(
        source,
        &[
            ExpectedHover {
                token: "Billing",
                occurrence: 0,
                ready_after: "module Billing",
                name: "Billing",
                display: "[Tyda] singleton(Billing)",
            },
            ExpectedHover {
                token: "TAX_RATE",
                occurrence: 0,
                ready_after: "  TAX_RATE = 0.1",
                name: "TAX_RATE",
                display: "[Tyda] 0.1",
            },
            ExpectedHover {
                token: "Invoice",
                occurrence: 0,
                ready_after: "  class Invoice",
                name: "Billing::Invoice",
                display: "[Tyda] singleton(Billing::Invoice)",
            },
            ExpectedHover {
                token: "total",
                occurrence: 1,
                ready_after: "      @total = total",
                name: "total",
                display: "[Tyda] Integer",
            },
            ExpectedHover {
                token: "@total",
                occurrence: 0,
                ready_after: "      @total = total",
                name: "@total",
                display: "[Tyda] Integer",
            },
            ExpectedHover {
                token: "local",
                occurrence: 0,
                ready_after: "      local = @total",
                name: "local",
                display: "[Tyda] Integer",
            },
            ExpectedHover {
                token: "amount",
                occurrence: 0,
                ready_after: "      local\n    end",
                name: "amount",
                display: "[Tyda] -> Integer",
            },
        ],
    );
}

#[test]
fn stable_hovers_survive_a_later_broken_method() {
    let source = "class Sample\n  def stable = 1\n  def broken = \"#{stable\nend\n";
    assert_incremental_hovers(
        source,
        &[
            ExpectedHover {
                token: "Sample",
                occurrence: 0,
                ready_after: "class Sample",
                name: "Sample",
                display: "[Tyda] singleton(Sample)",
            },
            ExpectedHover {
                token: "stable",
                occurrence: 0,
                ready_after: "  def stable = 1",
                name: "stable",
                display: "[Tyda] -> 1",
            },
        ],
    );
}
