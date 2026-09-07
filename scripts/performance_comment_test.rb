#!/usr/bin/env ruby

require "json"
require "minitest/autorun"
require "tmpdir"

require_relative "performance_comment"

class PerformanceCommentTest < Minitest::Test
  def test_builds_a_compact_subject_table
    Dir.mktmpdir do |dir|
      write_result(dir, "performance-result-rack", "rack", "passed", [
        metric("cli_elapsed_ms", "time", 100, 120, 20.0),
        metric("cli_max_rss_bytes", "memory", 10 * 1024 * 1024, 11 * 1024 * 1024, 10.0),
        metric("lsp_scan_ms", "time", 50, 45, -10.0),
        metric("lsp_max_rss_bytes", "memory", 8 * 1024 * 1024, 8 * 1024 * 1024, 0.0),
      ])

      body = PerformanceComment.build_comment(
        results_dir: dir,
        performance_result: "success",
        build_result: "success",
        run_url: "https://github.com/pvcresin/tyda/actions/runs/1",
      )

      assert_includes body, "<!-- tyda-performance-report -->"
      assert_includes body, "| Subject | CLI time (base → head)"
      assert_includes body, "| rack | 100 ms → 120 ms (+20.0%)"
      assert_includes body, "11.0 MiB (+10.0%)"
      assert_includes body, "⚠️ WARN"
      assert_includes body, "[View the workflow run]"
    end
  end

  def test_keeps_skipped_metrics_visible
    Dir.mktmpdir do |dir|
      write_result(dir, "performance-result-optcarrot", "optcarrot", "passed", [
        metric("cli_elapsed_ms", "time", 100, 100, 0.0),
        {
          "name" => "cli_max_rss_bytes",
          "kind" => "memory",
          "unit" => "bytes",
          "status" => "skipped",
          "reason" => "base timed out",
          "base_median" => nil,
          "head_median" => 10 * 1024 * 1024,
          "delta_percent" => nil,
          "warn" => false,
          "fail" => false,
        },
      ])

      body = PerformanceComment.build_comment(
        results_dir: dir,
        performance_result: "success",
        build_result: "success",
      )

      assert_includes body, "— (base timed out)"
      assert_includes body, "✅ OK (partial)"
      assert_includes body, "`SKIP` means the base sample was unavailable"
    end
  end

  def test_reports_when_the_path_classifier_skips_benchmarks
    Dir.mktmpdir do |dir|
      body = PerformanceComment.build_comment(
        results_dir: dir,
        performance_result: "skipped",
        build_result: "skipped",
        run_full_ci: "false",
      )

      assert_includes body, "changed paths do not require them"
      assert_includes body, "| — | — | — | — | — | ⏭️ SKIP |"
      refute_includes body, "❌ No result"
    end
  end

  private

  def metric(name, kind, base, head, delta)
    {
      "name" => name,
      "kind" => kind,
      "unit" => kind == "memory" ? "bytes" : "ms",
      "status" => "compared",
      "base_median" => base,
      "head_median" => head,
      "delta_percent" => delta,
      "warn" => delta > 15,
      "fail" => delta > 30,
    }
  end

  def write_result(dir, artifact, subject_name, status, metrics)
    artifact_dir = File.join(dir, artifact)
    Dir.mkdir(artifact_dir)
    File.write(
      File.join(artifact_dir, "result.json"),
      JSON.generate(
        "subject" => "/home/runner/work/tyda/tyda/subject/#{subject_name}",
        "runs" => 3,
        "base_sha" => "0123456789abcdef",
        "head_sha" => "fedcba9876543210",
        "metrics" => metrics,
        "status" => status,
      ),
    )
  end
end
