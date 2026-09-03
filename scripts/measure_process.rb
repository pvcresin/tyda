#!/usr/bin/env ruby

require "fiddle"
require "fileutils"
require "optparse"

RUSAGE_CHILDREN = -1
RUSAGE_MAXRSS_OFFSET = 32
GETRUSAGE = Fiddle::Function.new(
  Fiddle::Handle::DEFAULT.sym("getrusage"),
  [Fiddle::TYPE_INT, Fiddle::TYPE_VOIDP],
  Fiddle::TYPE_INT,
)

def maximum_rss_bytes
  usage = Fiddle::Pointer.malloc(256)
  result = GETRUSAGE.call(RUSAGE_CHILDREN, usage)
  raise "getrusage failed: #{result}" unless result.zero?

  maxrss = usage[RUSAGE_MAXRSS_OFFSET, Fiddle::SIZEOF_LONG].unpack1(
    Fiddle::SIZEOF_LONG == 8 ? "q" : "l",
  )
  RUBY_PLATFORM.include?("darwin") ? maxrss : maxrss * 1024
end

options = {}
parser = OptionParser.new do |opts|
  opts.on("--log PATH", String) { |value| options[:log] = value }
  opts.on("--output PATH", String) { |value| options[:output] = value }
  opts.on("--timeout SECONDS", Float) { |value| options[:timeout] = value }
end

separator = ARGV.index("--")
option_args = separator ? ARGV.take(separator) : ARGV.dup
command = separator ? ARGV.drop(separator + 1) : []
begin
  parser.parse!(option_args)
rescue OptionParser::ParseError => error
  warn error.message
  exit 2
end

missing = %i[log output timeout].reject { |key| options.key?(key) }
if missing.any?
  warn "missing required options: #{missing.join(", ")}"
  warn parser
  exit 2
elsif command.empty? || options[:timeout] <= 0
  warn parser
  warn "a positive timeout and a command after -- are required"
  exit 2
end

FileUtils.mkdir_p(File.dirname(options[:log]))
FileUtils.mkdir_p(File.dirname(options[:output]))
started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
status = 0

File.open(options[:log], "wb") do |log|
  pid = Process.spawn(*command, out: log, err: [:child, :out])
  loop do
    waited_pid, child_status = Process.waitpid2(pid, Process::WNOHANG)
    if waited_pid
      status = child_status.success? ? 0 : child_status.exitstatus || 1
      break
    end

    if Process.clock_gettime(Process::CLOCK_MONOTONIC) - started >= options[:timeout]
      begin
        Process.kill("KILL", pid)
      rescue Errno::ESRCH
        nil
      end
      Process.wait(pid)
      status = 124
      break
    end
    sleep 0.01
  end
end

elapsed_ms = ((Process.clock_gettime(Process::CLOCK_MONOTONIC) - started) * 1000).round
File.write(options[:output], "#{elapsed_ms}\t#{maximum_rss_bytes}\n")
exit status
