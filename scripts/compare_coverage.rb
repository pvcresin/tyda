#!/usr/bin/env ruby

require "fileutils"
require "json"
require "optparse"

module CoverageComparison
  REPORT_SCHEMA_VERSION = 1
  RESULT_SCHEMA_VERSION = 1
  COUNT_KEYS = %w[total typed untyped unknown].freeze

  class InvalidReport < StandardError; end

  class << self
    def read_report(path)
      report = JSON.parse(File.read(path))
      validate_report(report, path)
      report
    rescue JSON::ParserError => error
      raise InvalidReport, "#{path}: invalid JSON (#{error.message})"
    rescue Errno::ENOENT => error
      raise InvalidReport, error.message
    end

    def validate_report(report, path)
      unless report.is_a?(Hash) && report["schema_version"] == REPORT_SCHEMA_VERSION
        raise InvalidReport, "#{path}: unsupported coverage report schema"
      end

      files = report["files"]
      unless files.is_a?(Hash)
        raise InvalidReport, "#{path}: files must be an object"
      end
      %w[discovered analyzed].each do |key|
        validate_count(files[key], "#{path}: files.#{key}")
      end
      if files["analyzed"] > files["discovered"]
        raise InvalidReport, "#{path}: analyzed files exceed discovered files"
      end

      %w[declarations references].each do |key|
        validate_stats(report[key], "#{path}: #{key}")
      end
      by_kind = report["by_kind"]
      unless by_kind.is_a?(Hash)
        raise InvalidReport, "#{path}: by_kind must be an object"
      end
      by_kind.each do |kind, stats|
        validate_stats(stats, "#{path}: by_kind.#{kind}")
      end
    end

    def compare_reports(base_report, head_report, metadata, base_timeout: false)
      validate_report(head_report, "head")
      if base_timeout
        return {
          "comparison_schema_version" => RESULT_SCHEMA_VERSION,
          **metadata,
          "base_timeout" => true,
          "files" => { "base" => nil, "head" => head_report.fetch("files") },
          "scope_status" => "skipped",
          "metrics" => [],
          "warnings" => [],
          "failures" => [],
          "reason" => "base timed out",
          "status" => "skipped",
        }
      end

      validate_report(base_report, "base")
      file_result = compare_files(base_report.fetch("files"), head_report.fetch("files"))
      metrics = []
      %w[declarations references].each do |name|
        metrics << compare_metric(
          name,
          base_report.fetch(name),
          head_report.fetch(name),
        )
      end
      kinds = (base_report.fetch("by_kind").keys | head_report.fetch("by_kind").keys).sort
      kinds.each do |kind|
        metrics << compare_metric(
          "by_kind.#{kind}",
          base_report.fetch("by_kind")[kind],
          head_report.fetch("by_kind")[kind],
        )
      end

      warnings = file_result.fetch("warnings")
      failures = file_result.fetch("failures")
      metrics.each do |metric|
        metric.fetch("warnings").each { |message| warnings << message }
        metric.fetch("failures").each { |message| failures << message }
      end
      status = if failures.any?
        "failed"
      elsif warnings.any?
        "warned"
      else
        "passed"
      end

      {
        "comparison_schema_version" => RESULT_SCHEMA_VERSION,
        **metadata,
        "base_timeout" => false,
        "files" => file_result.slice("base", "head", "delta"),
        "scope_status" => file_result.fetch("status"),
        "metrics" => metrics,
        "warnings" => warnings,
        "failures" => failures,
        "status" => status,
      }
    end

    private

    def validate_stats(stats, path)
      unless stats.is_a?(Hash)
        raise InvalidReport, "#{path} must be an object"
      end
      counts = COUNT_KEYS.to_h do |key|
        [key, validate_count(stats[key], "#{path}.#{key}")]
      end
      unless counts.values_at("typed", "untyped", "unknown").sum == counts.fetch("total")
        raise InvalidReport, "#{path}: counts do not add up to total"
      end
      %w[type_coverage_percent tracking_coverage_percent].each do |key|
        value = stats[key]
        unless value.is_a?(Numeric) && value >= 0
          raise InvalidReport, "#{path}.#{key} must be a non-negative number"
        end
      end
      counts
    end

    def validate_count(value, path)
      unless value.is_a?(Integer) && value >= 0
        raise InvalidReport, "#{path} must be a non-negative integer"
      end
      value
    end

    def counts_for(stats, name)
      return { "total" => 0, "typed" => 0, "untyped" => 0, "unknown" => 0 } unless stats

      validate_stats(stats, name)
      COUNT_KEYS.to_h { |key| [key, stats.fetch(key)] }
    end

    def compare_files(base, head)
      warnings = []
      failures = []
      delta = {}
      %w[discovered analyzed].each do |key|
        change = head.fetch(key) - base.fetch(key)
        delta[key] = change
        if change.negative?
          failures << "files.#{key} decreased by #{change.abs}"
        elsif change.positive?
          warnings << "files.#{key} increased by #{change}"
        end
      end
      if base.fetch("analyzed") < base.fetch("discovered")
        failures << "base analysis is incomplete"
      end
      if head.fetch("analyzed") < head.fetch("discovered")
        failures << "head analysis is incomplete"
      end
      {
        "base" => base,
        "head" => head,
        "delta" => delta,
        "status" => failures.any? ? "failed" : warnings.any? ? "warned" : "passed",
        "warnings" => warnings,
        "failures" => failures,
      }
    end

    def compare_metric(name, base_stats, head_stats)
      base = counts_for(base_stats, "base #{name}")
      head = counts_for(head_stats, "head #{name}")
      base_tracking = base.fetch("typed") + base.fetch("untyped")
      head_tracking = head.fetch("typed") + head.fetch("untyped")
      failures = []
      warnings = []

      if head.fetch("total") < base.fetch("total")
        failures << "#{name}: total decreased by #{base.fetch("total") - head.fetch("total")}"
      end
      if head.fetch("typed") < base.fetch("typed")
        failures << "#{name}: typed decreased by #{base.fetch("typed") - head.fetch("typed")}"
      end
      if head_tracking < base_tracking
        failures << "#{name}: tracked decreased by #{base_tracking - head_tracking}"
      end
      if head.fetch("unknown") > base.fetch("unknown")
        failures << "#{name}: unknown increased by #{head.fetch("unknown") - base.fetch("unknown")}"
      end
      if head.fetch("untyped") > base.fetch("untyped")
        warnings << "#{name}: untyped increased by #{head.fetch("untyped") - base.fetch("untyped")}"
      end

      {
        "name" => name,
        "status" => failures.any? ? "failed" : warnings.any? ? "warned" : "passed",
        "base" => metric_payload(base),
        "head" => metric_payload(head),
        "delta" => {
          "total" => head.fetch("total") - base.fetch("total"),
          "typed" => head.fetch("typed") - base.fetch("typed"),
          "untyped" => head.fetch("untyped") - base.fetch("untyped"),
          "unknown" => head.fetch("unknown") - base.fetch("unknown"),
          "tracking" => head_tracking - base_tracking,
          "type_coverage_percent" => percentage(head.fetch("typed"), head.fetch("total")) -
            percentage(base.fetch("typed"), base.fetch("total")),
          "tracking_coverage_percent" => percentage(head_tracking, head.fetch("total")) -
            percentage(base_tracking, base.fetch("total")),
        },
        "warnings" => warnings,
        "failures" => failures,
      }
    end

    def metric_payload(counts)
      tracking = counts.fetch("typed") + counts.fetch("untyped")
      {
        **counts,
        "tracking" => tracking,
        "type_coverage_percent" => percentage(counts.fetch("typed"), counts.fetch("total")).round(2),
        "tracking_coverage_percent" => percentage(tracking, counts.fetch("total")).round(2),
      }
    end

    def percentage(value, total)
      return 0.0 if total.zero?

      value * 100.0 / total
    end
  end
end

if $PROGRAM_NAME == __FILE__
  options = {}
  parser = OptionParser.new do |opts|
    opts.on("--base PATH", String) { |value| options[:base] = value }
    opts.on("--base-sha SHA", String) { |value| options[:base_sha] = value }
    opts.on("--base-timeout") { options[:base_timeout] = true }
    opts.on("--head PATH", String) { |value| options[:head] = value }
    opts.on("--head-sha SHA", String) { |value| options[:head_sha] = value }
    opts.on("--output PATH", String) { |value| options[:output] = value }
    opts.on("--subject PATH", String) { |value| options[:subject] = value }
    opts.on("--subject-ref SHA", String) { |value| options[:subject_ref] = value }
  end

  begin
    parser.parse!(ARGV)
  rescue OptionParser::ParseError => error
    warn error.message
    exit 2
  end

  required = %i[base head output subject]
  missing = required.reject { |key| options.key?(key) }
  unless missing.empty?
    warn "missing required options: #{missing.join(", ")}"
    warn parser
    exit 2
  end

  begin
    head_report = CoverageComparison.read_report(options.fetch(:head))
    base_report = if options[:base_timeout]
      nil
    else
      CoverageComparison.read_report(options.fetch(:base))
    end
    result = CoverageComparison.compare_reports(
      base_report,
      head_report,
      {
        "subject" => options.fetch(:subject),
        "subject_ref" => options.fetch(:subject_ref, "unknown"),
        "base_sha" => options.fetch(:base_sha, "unknown"),
        "head_sha" => options.fetch(:head_sha, "unknown"),
      },
      base_timeout: options.fetch(:base_timeout, false),
    )
  rescue CoverageComparison::InvalidReport => error
    warn error.message
    exit 1
  end

  puts "=== Coverage gate ==="
  puts "subject: #{result.fetch("subject")}"
  puts "base: #{result.fetch("base_sha")}"
  puts "head: #{result.fetch("head_sha")}"
  if result.fetch("status") == "skipped"
    puts "SKIP base timed out; coverage comparison unavailable"
  else
    files = result.fetch("files")
    file_status = result.fetch("scope_status").upcase
    puts format(
      "%-4s files: discovered %d -> %d, analyzed %d -> %d",
      file_status,
      files.fetch("base").fetch("discovered"),
      files.fetch("head").fetch("discovered"),
      files.fetch("base").fetch("analyzed"),
      files.fetch("head").fetch("analyzed"),
    )
      result.fetch("metrics").each do |metric|
      base = metric.fetch("base")
      head = metric.fetch("head")
      delta = metric.fetch("delta")
      puts format(
        "%-4s %-27s typed %d -> %d (%+.2fpp), tracked %d -> %d (%+.2fpp), unknown %+d, untyped %+d",
        metric.fetch("status").upcase,
        metric.fetch("name"),
        base.fetch("typed"),
        head.fetch("typed"),
        delta.fetch("type_coverage_percent"),
        base.fetch("tracking"),
        head.fetch("tracking"),
        delta.fetch("tracking_coverage_percent"),
        delta.fetch("unknown"),
        delta.fetch("untyped"),
      )
    end
    result.fetch("warnings").each { |message| puts "WARN #{message}" }
    result.fetch("failures").each { |message| puts "FAIL #{message}" }
  end

  FileUtils.mkdir_p(File.dirname(options.fetch(:output)))
  File.write(options.fetch(:output), JSON.pretty_generate(result) + "\n")
  exit(result.fetch("status") == "failed" ? 1 : 0)
end
