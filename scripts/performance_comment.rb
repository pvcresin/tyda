#!/usr/bin/env ruby

require "fileutils"
require "json"
require "optparse"

module PerformanceComment
  MARKER = "<!-- tyda-performance-report -->"
  METRICS = [
    ["cli_elapsed_ms", "CLI time"],
    ["cli_max_rss_bytes", "CLI max RSS"],
    ["lsp_scan_ms", "LSP scan"],
    ["lsp_max_rss_bytes", "LSP max RSS"],
  ].freeze

  module_function

  def build_comment(results_dir:, performance_result:, build_result:, run_full_ci: "true", run_url: nil)
    reports = load_reports(results_dir)
    base_sha = reports.map { |report| report.fetch("base_sha", nil) }.compact.uniq.first
    head_sha = reports.map { |report| report.fetch("head_sha", nil) }.compact.uniq.first
    first_report = reports.first

    lines = [MARKER, "## Performance comparison", ""]
    lines << "Comparing `#{short_sha(base_sha)}` → `#{short_sha(head_sha)}` " \
      "with paired benchmark runs (median per metric)."
    lines << "[View the workflow run](#{run_url})" if run_url && !run_url.empty?
    lines << ""

    if build_result != "success"
      lines << "> ⚠️ The performance build concluded with `#{build_result}`."
      lines << ""
    end
    if performance_result != "success"
      lines << "> ⚠️ At least one performance subject job concluded with `#{performance_result}`."
      lines << ""
    end
    if run_full_ci != "true"
      lines << "> ⏭️ Performance benchmarks were skipped because the changed paths do not require them."
      lines << ""
    end

    lines << "| Subject | CLI time (base → head) | CLI max RSS (base → head) | LSP scan (base → head) | LSP max RSS (base → head) | Result |"
    lines << "| --- | ---: | ---: | ---: | ---: | :--- |"
    if reports.empty?
      result = run_full_ci == "true" ? "❌ No result" : "⏭️ SKIP"
      lines << "| — | — | — | — | — | #{result} |"
    else
      reports.each do |report|
        metrics = report.fetch("metrics", [])
        lines << [
          report.fetch("subject_name"),
          metric_cell(metrics, "cli_elapsed_ms"),
          metric_cell(metrics, "cli_max_rss_bytes"),
          metric_cell(metrics, "lsp_scan_ms"),
          metric_cell(metrics, "lsp_max_rss_bytes"),
          report_status(report),
        ].join(" | ").prepend("| ").concat(" |")
      end
    end

    lines << ""
    lines << "<details>"
    lines << "<summary>Measurement details</summary>"
    lines << ""
    lines << "- Full benchmark run: `#{run_full_ci}`; performance jobs: `#{performance_result}`; build job: `#{build_result}`."
    lines << "- Runs: `#{first_report&.fetch("runs", "unknown") || "unknown"}` paired runs; values are medians."
    lines << "- Time: WARN above `15%`; FAIL above `30%` and at least `100 ms`."
    lines << "- Memory: WARN above `10%`; FAIL above `20%` and at least `16 MiB`."
    lines << "- `SKIP` means the base sample was unavailable, such as an accepted base timeout."
    lines << ""
    lines << "</details>"
    lines.join("\n") + "\n"
  end

  def load_reports(results_dir)
    paths = Dir.glob(File.join(results_dir, "**", "result.json"))
    paths.filter_map do |path|
      report = JSON.parse(File.read(path))
      report["subject_name"] ||= subject_name(path, report)
      report
    rescue JSON::ParserError, Errno::ENOENT => error
      warn "skipping invalid performance result #{path}: #{error.message}"
      nil
    end.sort_by { |report| report.fetch("subject_name") }
  end

  def subject_name(path, report)
    artifact_name = File.basename(File.dirname(path)).sub(/\Aperformance-result-/, "")
    return artifact_name unless artifact_name.empty? || artifact_name == "performance-results"

    subject = report.fetch("subject", "unknown")
    subject.split("/subject/", 2).last.split("/", 2).first
  end

  def metric_cell(metrics, name)
    metric = metrics.find { |candidate| candidate["name"] == name }
    return "—" unless metric

    if metric["status"] == "skipped"
      return "— (#{metric.fetch("reason", "skipped")})"
    end

    base = format_value(metric.fetch("base_median"), metric.fetch("unit"))
    head = format_value(metric.fetch("head_median"), metric.fetch("unit"))
    delta = format("%+.1f%%", metric.fetch("delta_percent").to_f)
    "#{base} → #{head} (#{delta})"
  end

  def format_value(value, unit)
    return "—" if value.nil?

    if unit == "bytes"
      format("%.1f MiB", value.to_f / (1024 * 1024))
    elsif value.to_f == value.to_i
      "#{value.to_i} ms"
    else
      format("%.1f ms", value)
    end
  end

  def report_status(report)
    metrics = report.fetch("metrics", [])
    return "❌ FAIL" if report["status"] == "failed" || metrics.any? { |metric| metric["fail"] }
    return "⚠️ WARN" if metrics.any? { |metric| metric["warn"] }
    return "⏭️ SKIP" if metrics.empty? || metrics.all? { |metric| metric["status"] == "skipped" }
    return "✅ OK (partial)" if metrics.any? { |metric| metric["status"] == "skipped" }

    "✅ OK"
  end

  def short_sha(sha)
    sha ? sha[0, 12] : "unknown"
  end
end

if $PROGRAM_NAME == __FILE__
  options = {
    results_dir: ENV.fetch("TYDA_PERF_RESULTS_DIR", "target/performance-results"),
    output: ENV.fetch("TYDA_PERF_COMMENT_PATH", "target/performance-comment.md"),
    performance_result: ENV.fetch("TYDA_PERF_JOB_RESULT", "unknown"),
    build_result: ENV.fetch("TYDA_PERF_BUILD_RESULT", "unknown"),
    run_full_ci: ENV.fetch("TYDA_PERF_RUN_FULL_CI", "true"),
    run_url: ENV["TYDA_PERF_RUN_URL"],
  }
  parser = OptionParser.new do |opts|
    opts.on("--results-dir PATH", String) { |value| options[:results_dir] = value }
    opts.on("--output PATH", String) { |value| options[:output] = value }
  end
  parser.parse!(ARGV)

  body = PerformanceComment.build_comment(
    results_dir: options[:results_dir],
    performance_result: options[:performance_result],
    build_result: options[:build_result],
    run_full_ci: options[:run_full_ci],
    run_url: options[:run_url],
  )
  FileUtils.mkdir_p(File.dirname(options[:output]))
  File.write(options[:output], body)
  puts body
end
