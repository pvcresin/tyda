#!/usr/bin/env ruby
# frozen_string_literal: true

require "fileutils"
require "json"
require "minitest/autorun"
require "tmpdir"

require_relative "summarize_analysis"

class SummarizeAnalysisTest < Minitest::Test
  def test_subject_summary_reports_each_check
    Dir.mktmpdir do |dir|
      write(dir, "output/rbs.result", "changed\n")
      write(dir, "output/rbs.diff", "--- base\n+++ head\n@@ -1 +1,2 @@\n-def a: -> untyped\n+def a: -> Integer\n+def b: -> String\n")
      write(dir, "output/diagnostics.result", "identical\n")
      write(dir, "coverage/result.json", JSON.generate(coverage_result))

      summary = AnalysisSummary.subject_summary(dir, "rack", approved: false)

      assert_equal({ "status" => "changed", "added" => 2, "removed" => 1, "diff" => "output/rbs.diff" }, summary.fetch("rbs"))
      assert_equal({ "status" => "identical" }, summary.fetch("diagnostics"))
      coverage = summary.fetch("coverage")
      assert_equal "failed", coverage.fetch("status")
      assert_equal({ "declarations" => -3, "references" => 4 }, coverage.fetch("typed_delta"))
      assert_equal ["declarations: typed decreased by 3"], coverage.fetch("failures")
    end
  end

  def test_missing_results_are_reported_as_missing
    Dir.mktmpdir do |dir|
      summary = AnalysisSummary.subject_summary(dir, "rack", approved: true)

      assert_equal "missing", summary.fetch("rbs").fetch("status")
      assert_equal "missing", summary.fetch("diagnostics").fetch("status")
      assert_equal "missing", summary.fetch("coverage").fetch("status")
    end
  end

  def test_subject_markdown_escapes_diff_preview
    Dir.mktmpdir do |dir|
      write(dir, "output/rbs.result", "changed\n")
      write(dir, "output/rbs.diff", "+def a: -> Array[<untyped>]\n")
      write(dir, "output/diagnostics.result", "identical\n")
      summary = AnalysisSummary.subject_summary(dir, "rack", approved: false)

      markdown = AnalysisSummary.subject_markdown(summary, dir, run_id: "42")

      assert_includes markdown, "Array[&lt;untyped&gt;]"
      assert_includes markdown, "gh run download 42 -n analysis-comparison-rack"
    end
  end

  def test_aggregate_markdown_has_one_row_per_subject
    summaries = [
      subject("redmine", rbs: { "status" => "changed", "added" => 5, "removed" => 2 }),
      subject("rack", rbs: { "status" => "identical" }),
    ]

    markdown = AnalysisSummary.aggregate_markdown(summaries)

    rows = markdown.lines.grep(/\A\| (rack|redmine) /)
    assert_equal ["rack", "redmine"], rows.map { |row| row.split("|")[1].strip }
    assert_includes markdown, "| redmine | changed (+5 / -2 lines) | identical | passed | +0 / -1 |"
  end

  private

  def write(dir, path, content)
    full_path = File.join(dir, path)
    FileUtils.mkdir_p(File.dirname(full_path))
    File.write(full_path, content)
  end

  def coverage_result
    {
      "status" => "failed",
      "metrics" => [
        { "name" => "declarations", "delta" => { "typed" => -3 } },
        { "name" => "references", "delta" => { "typed" => 4 } },
      ],
      "failures" => ["declarations: typed decreased by 3"],
      "warnings" => [],
    }
  end

  def subject(name, rbs:)
    {
      "subject" => name,
      "approved" => false,
      "rbs" => rbs,
      "diagnostics" => { "status" => "identical" },
      "coverage" => { "status" => "passed", "typed_delta" => { "declarations" => 0, "references" => -1 } },
    }
  end
end
