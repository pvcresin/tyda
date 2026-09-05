#!/usr/bin/env ruby

require "json"
require "fileutils"
require "optparse"

METRICS = [
  ["cli_elapsed_ms", "time", "ms"],
  ["cli_max_rss_bytes", "memory", "bytes"],
  ["lsp_scan_ms", "time", "ms"],
  ["lsp_max_rss_bytes", "memory", "bytes"],
].freeze

options = {}
parser = OptionParser.new do |opts|
  opts.on("--base-sha SHA", String) { |value| options[:base_sha] = value }
  opts.on("--head-sha SHA", String) { |value| options[:head_sha] = value }
  opts.on("--metrics PATH", String) { |value| options[:metrics] = value }
  opts.on("--output PATH", String) { |value| options[:output] = value }
  opts.on("--runs N", Integer) { |value| options[:runs] = value }
  opts.on("--subject PATH", String) { |value| options[:subject] = value }
  opts.on("--time-fail-percent N", Float) { |value| options[:time_fail_percent] = value }
  opts.on("--time-warn-percent N", Float) { |value| options[:time_warn_percent] = value }
  opts.on("--memory-fail-percent N", Float) { |value| options[:memory_fail_percent] = value }
  opts.on("--memory-warn-percent N", Float) { |value| options[:memory_warn_percent] = value }
  opts.on("--min-time-delta-ms N", Integer) { |value| options[:min_time_delta_ms] = value }
  opts.on("--min-memory-delta-bytes N", Integer) { |value| options[:min_memory_delta_bytes] = value }
end

begin
  parser.parse!(ARGV)
rescue OptionParser::ParseError => error
  warn error.message
  exit 2
end

required = %i[
  base_sha
  head_sha
  metrics
  output
  runs
  subject
  time_fail_percent
  time_warn_percent
  memory_fail_percent
  memory_warn_percent
  min_time_delta_ms
  min_memory_delta_bytes
]
missing = required.reject { |key| options.key?(key) }
unless missing.empty?
  warn "missing required options: #{missing.join(", ")}"
  exit 2
end

measurements = METRICS.to_h { |metric, _kind, _unit| [metric, { "base" => [], "head" => [] }] }
File.foreach(options[:metrics]) do |line|
  next if line.strip.empty?

  _run, variant, metric, value = line.chomp.split("\t", -1)
  measurements.fetch(metric).fetch(variant) << Integer(value, 10)
end

METRICS.each do |metric, _kind, _unit|
  values = measurements.fetch(metric)
  %w[base head].each do |variant|
    actual = values.fetch(variant).length
    next if actual == options[:runs]

    abort "expected #{options[:runs]} #{variant} samples for #{metric}, got #{actual}"
  end
end

def median(values)
  sorted = values.sort
  middle = sorted.length / 2
  return sorted[middle] if sorted.length.odd?

  ((sorted[middle - 1] + sorted[middle]) / 2.0).round
end

def format_value(value, unit)
  return format("%.1f MiB", value.fdiv(1024 * 1024)) if unit == "bytes"

  "#{value} ms"
end

results = []
failed = false
warned = false

METRICS.each do |metric, kind, unit|
  values = measurements.fetch(metric)
  if kind == "memory" && values.fetch("base").any?(&:negative?)
    puts format("SKIP %-20s base timed out; memory comparison unavailable", metric)
    results << {
      "name" => metric,
      "kind" => kind,
      "unit" => unit,
      "status" => "skipped",
      "reason" => "base timed out",
      "base_samples" => [],
      "head_samples" => values.fetch("head"),
      "base_median" => nil,
      "head_median" => nil,
      "delta_percent" => nil,
      "warn" => false,
      "fail" => false,
      "warn_percent" => options[:memory_warn_percent],
      "fail_percent" => options[:memory_fail_percent],
      "minimum_delta" => options[:min_memory_delta_bytes],
    }
    next
  end

  base_median = median(values.fetch("base"))
  head_median = median(values.fetch("head"))
  delta = head_median - base_median
  delta_percent = base_median.zero? ? 0.0 : delta * 100.0 / base_median

  if kind == "time"
    warn_percent = options[:time_warn_percent]
    fail_percent = options[:time_fail_percent]
    minimum_delta = options[:min_time_delta_ms]
  else
    warn_percent = options[:memory_warn_percent]
    fail_percent = options[:memory_fail_percent]
    minimum_delta = options[:min_memory_delta_bytes]
  end

  warn_regression = delta.positive? && delta_percent > warn_percent && delta >= minimum_delta
  fail_regression = delta.positive? && delta_percent > fail_percent && delta >= minimum_delta
  warned ||= warn_regression
  failed ||= fail_regression
  status = fail_regression ? "FAIL" : warn_regression ? "WARN" : "OK"
  puts format(
    "%-4s %-20s base=%12s head=%12s delta=%+6.1f%%",
    status,
    metric,
    format_value(base_median, unit),
    format_value(head_median, unit),
    delta_percent,
  )

  if warn_regression
    message = format(
      "%s increased by %.1f%% (%s -> %s)",
      metric,
      delta_percent,
      format_value(base_median, unit),
      format_value(head_median, unit),
    )
    annotation = fail_regression ? "error title=Performance regression" : "warning title=Performance drift"
    puts "::#{annotation}::#{message}"
  end

  results << {
    "name" => metric,
    "kind" => kind,
    "unit" => unit,
    "status" => "compared",
    "base_samples" => values.fetch("base"),
    "head_samples" => values.fetch("head"),
    "base_median" => base_median,
    "head_median" => head_median,
    "delta_percent" => delta_percent.round(2),
    "warn" => warn_regression,
    "fail" => fail_regression,
    "warn_percent" => warn_percent,
    "fail_percent" => fail_percent,
    "minimum_delta" => minimum_delta,
  }
end

output = {
  "subject" => options[:subject],
  "runs" => options[:runs],
  "base_sha" => options[:base_sha],
  "head_sha" => options[:head_sha],
  "thresholds" => {
    "time_warn_percent" => options[:time_warn_percent],
    "time_fail_percent" => options[:time_fail_percent],
    "memory_warn_percent" => options[:memory_warn_percent],
    "memory_fail_percent" => options[:memory_fail_percent],
    "min_time_delta_ms" => options[:min_time_delta_ms],
    "min_memory_delta_bytes" => options[:min_memory_delta_bytes],
  },
  "metrics" => results,
  "status" => failed ? "failed" : "passed",
}
FileUtils.mkdir_p(File.dirname(options[:output]))
File.write(options[:output], JSON.pretty_generate(output) + "\n")

puts "Performance drift is within the allowed CI tolerance." if warned && !failed
puts "Performance regression exceeded the CI tolerance." if failed
exit(failed ? 1 : 0)
