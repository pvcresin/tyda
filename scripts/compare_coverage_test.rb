#!/usr/bin/env ruby

require "minitest/autorun"

require_relative "compare_coverage"

class CompareCoverageTest < Minitest::Test
  def test_unchanged_reports_pass
    report = coverage_report(typed: 2, untyped: 1, unknown: 1)

    result = compare(report, report)

    assert_equal "passed", result.fetch("status")
    assert_empty result.fetch("warnings")
    assert_empty result.fetch("failures")
  end

  def test_typed_site_loss_fails
    base = coverage_report(typed: 3, untyped: 1, unknown: 0)
    head = coverage_report(typed: 2, untyped: 2, unknown: 0)

    result = compare(base, head)

    assert_equal "failed", result.fetch("status")
    assert result.fetch("failures").any? { |message| message.include?("typed decreased") }
  end

  def test_unknown_site_increase_fails
    base = coverage_report(typed: 2, untyped: 1, unknown: 1)
    head = coverage_report(typed: 2, untyped: 1, unknown: 2)

    result = compare(base, head)

    assert_equal "failed", result.fetch("status")
    assert result.fetch("failures").any? { |message| message.include?("unknown increased") }
  end

  def test_unknown_to_untyped_is_a_warning
    base = coverage_report(typed: 2, untyped: 1, unknown: 1)
    head = coverage_report(typed: 2, untyped: 2, unknown: 0)

    result = compare(base, head)

    assert_equal "warned", result.fetch("status")
    assert_empty result.fetch("failures")
    assert result.fetch("warnings").any? { |message| message.include?("untyped increased") }
  end

  def test_base_timeout_skips_comparison
    head = coverage_report(typed: 2, untyped: 1, unknown: 0)

    result = CoverageComparison.compare_reports(
      nil,
      head,
      metadata,
      base_timeout: true,
    )

    assert_equal "skipped", result.fetch("status")
    assert_equal "base timed out", result.fetch("reason")
  end

  private

  def compare(base, head)
    CoverageComparison.compare_reports(base, head, metadata)
  end

  def metadata
    {
      "subject" => "subject/sample",
      "subject_ref" => "subject-sha",
      "base_sha" => "base-sha",
      "head_sha" => "head-sha",
    }
  end

  def coverage_report(typed:, untyped:, unknown:)
    stats = coverage_stats(typed: typed, untyped: untyped, unknown: unknown)
    {
      "schema_version" => 1,
      "files" => { "discovered" => 1, "analyzed" => 1 },
      "declarations" => stats,
      "references" => stats,
      "by_kind" => { "value" => stats },
    }
  end

  def coverage_stats(typed:, untyped:, unknown:)
    total = typed + untyped + unknown
    {
      "total" => total,
      "typed" => typed,
      "untyped" => untyped,
      "unknown" => unknown,
      "type_coverage_percent" => total.zero? ? 0.0 : typed * 100.0 / total,
      "tracking_coverage_percent" => total.zero? ? 0.0 : (typed + untyped) * 100.0 / total,
    }
  end
end
