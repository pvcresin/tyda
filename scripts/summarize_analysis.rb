#!/usr/bin/env ruby
# frozen_string_literal: true

# Summarize analysis compatibility results for humans (Markdown) and agents (JSON).
#
#   summarize_analysis.rb subject DIR SUBJECT_NAME
#     Reads DIR/output/{rbs,diagnostics}.{result,diff} and DIR/coverage/result.json,
#     then writes DIR/summary.json and DIR/summary.md.
#   summarize_analysis.rb aggregate DIR
#     Reads every DIR/**/summary.json and prints one table for all subjects.
#
# Both commands append their Markdown to $GITHUB_STEP_SUMMARY when it is set.

require "erb"
require "json"

module AnalysisSummary
  SCHEMA_VERSION = 1
  OUTPUT_MODES = { "rbs" => "RBS output", "diagnostics" => "Diagnostics" }.freeze
  COVERAGE_METRICS = %w[declarations references].freeze
  DIFF_PREVIEW_LINES = 200
  FAILURE_PREVIEW = 20

  module_function

  def subject_summary(dir, subject, approved:)
    summary = {
      "schema_version" => SCHEMA_VERSION,
      "subject" => subject,
      "approved" => approved,
    }
    OUTPUT_MODES.each_key { |mode| summary[mode] = output_summary(dir, mode) }
    summary["coverage"] = coverage_summary(dir)
    summary
  end

  def output_summary(dir, mode)
    result_path = File.join(dir, "output", "#{mode}.result")
    status = File.exist?(result_path) ? File.read(result_path).strip : "missing"
    entry = { "status" => status }
    diff_path = File.join(dir, "output", "#{mode}.diff")
    if status == "changed" && File.exist?(diff_path)
      added = 0
      removed = 0
      File.foreach(diff_path) do |line|
        next if line.start_with?("+++", "---")

        added += 1 if line.start_with?("+")
        removed += 1 if line.start_with?("-")
      end
      entry["added"] = added
      entry["removed"] = removed
      entry["diff"] = "output/#{mode}.diff"
    end
    entry
  end

  def coverage_summary(dir)
    result_path = File.join(dir, "coverage", "result.json")
    return { "status" => "missing" } unless File.exist?(result_path)

    result = JSON.parse(File.read(result_path))
    metrics = result.fetch("metrics").to_h { |metric| [metric.fetch("name"), metric] }
    {
      "status" => result.fetch("status"),
      "typed_delta" => COVERAGE_METRICS.to_h do |name|
        [name, metrics[name]&.dig("delta", "typed")]
      end,
      "failures" => result.fetch("failures"),
      "warnings" => result.fetch("warnings"),
      "result" => "coverage/result.json",
    }
  end

  def subject_markdown(summary, dir, run_id: nil)
    lines = ["## Analysis: #{summary.fetch("subject")}", ""]
    lines << "Approval label: #{summary.fetch("approved") ? "present" : "absent"}"
    lines << ""
    lines << "| Check | Result | Details |"
    lines << "|---|---|---|"
    OUTPUT_MODES.each do |mode, label|
      entry = summary.fetch(mode)
      lines << "| #{label} | #{entry.fetch("status")} | #{output_details(entry)} |"
    end
    coverage = summary.fetch("coverage")
    lines << "| Coverage | #{coverage.fetch("status")} | #{typed_delta_text(coverage)} |"

    failures = coverage.fetch("failures", [])
    unless failures.empty?
      lines << ""
      lines << "Coverage failures:"
      failures.first(FAILURE_PREVIEW).each { |message| lines << "- #{message}" }
      lines << "- ... #{failures.size - FAILURE_PREVIEW} more" if failures.size > FAILURE_PREVIEW
    end

    OUTPUT_MODES.each do |mode, label|
      entry = summary.fetch(mode)
      next unless entry["diff"]

      diff = File.foreach(File.join(dir, entry.fetch("diff"))).first(DIFF_PREVIEW_LINES).join
      lines << ""
      lines << "<details><summary>#{label} diff (first #{DIFF_PREVIEW_LINES} lines)</summary>"
      lines << ""
      lines << "<pre>"
      lines << ERB::Util.html_escape(diff)
      lines << "</pre>"
      lines << ""
      lines << "</details>"
    end

    lines << ""
    lines << "The job log has each diff in a collapsed group. The full diffs and `summary.json` are in the " \
             "`analysis-comparison-#{summary.fetch("subject")}` artifact#{download_hint(run_id, summary.fetch("subject"))}."
    lines.join("\n") + "\n"
  end

  def aggregate_markdown(summaries, run_id: nil)
    lines = ["## Analysis compatibility", ""]
    if summaries.empty?
      lines << "No subject summaries were found."
      return lines.join("\n") + "\n"
    end

    approved = summaries.any? { |summary| summary.fetch("approved") }
    lines << "Approval label: #{approved ? "present" : "absent"}. " \
             "The label applies only to these inference checks, not to performance."
    lines << ""
    lines << "| Subject | RBS output | Diagnostics | Coverage | Typed delta (declarations / references) |"
    lines << "|---|---|---|---|---|"
    summaries.sort_by { |summary| summary.fetch("subject") }.each do |summary|
      rbs = summary.fetch("rbs")
      diagnostics = summary.fetch("diagnostics")
      coverage = summary.fetch("coverage")
      delta = coverage.fetch("typed_delta", {})
      lines << "| #{summary.fetch("subject")} | #{output_cell(rbs)} | #{output_cell(diagnostics)} | " \
               "#{coverage.fetch("status")} | #{signed(delta["declarations"])} / #{signed(delta["references"])} |"
    end
    lines << ""
    lines << "Each `analysis (<subject>)` job has the diffs in its log and summary. " \
             "Agents can read `summary.json` in each `analysis-comparison-<subject>` artifact" \
             "#{run_id ? " (`gh run download #{run_id} -p 'analysis-comparison-*'`)" : ""}."
    lines.join("\n") + "\n"
  end

  def output_details(entry)
    return "" unless entry.key?("added")

    "+#{entry.fetch("added")} / -#{entry.fetch("removed")} lines"
  end

  def output_cell(entry)
    details = output_details(entry)
    details.empty? ? entry.fetch("status") : "#{entry.fetch("status")} (#{details})"
  end

  def typed_delta_text(coverage)
    delta = coverage["typed_delta"]
    return "" unless delta

    "typed declarations #{signed(delta["declarations"])}, references #{signed(delta["references"])}"
  end

  def signed(value)
    return "-" if value.nil?

    format("%+d", value)
  end

  def download_hint(run_id, subject)
    run_id ? " (`gh run download #{run_id} -n analysis-comparison-#{subject}`)" : ""
  end

  def append_step_summary(markdown)
    path = ENV["GITHUB_STEP_SUMMARY"]
    File.write(path, markdown, mode: "a") if path && !path.empty?
  end
end

if $PROGRAM_NAME == __FILE__
  command, dir, subject = ARGV
  run_id = ENV["GITHUB_RUN_ID"]
  case command
  when "subject"
    unless dir && subject
      warn "usage: #{$PROGRAM_NAME} subject DIR SUBJECT_NAME"
      exit 2
    end
    approved = ENV.fetch("TYDA_ANALYSIS_APPROVED", "0") == "1"
    summary = AnalysisSummary.subject_summary(dir, subject, approved: approved)
    markdown = AnalysisSummary.subject_markdown(summary, dir, run_id: run_id)
    File.write(File.join(dir, "summary.json"), JSON.pretty_generate(summary) + "\n")
    File.write(File.join(dir, "summary.md"), markdown)
  when "aggregate"
    unless dir
      warn "usage: #{$PROGRAM_NAME} aggregate DIR"
      exit 2
    end
    summaries = Dir.glob(File.join(dir, "**", "summary.json")).map { |path| JSON.parse(File.read(path)) }
    markdown = AnalysisSummary.aggregate_markdown(summaries, run_id: run_id)
  else
    warn "usage: #{$PROGRAM_NAME} <subject|aggregate> ..."
    exit 2
  end
  AnalysisSummary.append_step_summary(markdown)
  puts markdown
end
