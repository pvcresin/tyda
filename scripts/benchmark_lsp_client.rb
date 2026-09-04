#!/usr/bin/env ruby

require "json"
require "open3"
require "socket"
require "timeout"
require "uri"

STARTUP_TIMEOUT_SECONDS = 30
MESSAGE_TIMEOUT_SECONDS = 180

def monotonic_seconds
  Process.clock_gettime(Process::CLOCK_MONOTONIC)
end

def file_uri(path)
  escaped = URI::DEFAULT_PARSER.escape(File.expand_path(path))
  URI::Generic.build(scheme: "file", path: escaped).to_s
end

def send_message(socket, message)
  body = JSON.generate(message)
  socket.write("Content-Length: #{body.bytesize}\r\n\r\n")
  socket.write(body)
  socket.flush
end

def read_message(socket, timeout_seconds)
  headers = {}
  loop do
    ready = IO.select([socket], nil, nil, timeout_seconds)
    raise "timed out waiting for an LSP message" unless ready

    line = socket.gets
    raise "LSP connection closed while reading headers" unless line
    break if line == "\r\n" || line == "\n"

    key, value = line.split(":", 2)
    headers[key.downcase] = value.to_s.strip
  end

  length = Integer(headers.fetch("content-length"), 10)
  body = Timeout.timeout(timeout_seconds) { socket.read(length) }
  raise "LSP connection closed while reading body" unless body&.bytesize == length

  JSON.parse(body)
end

def wait_for_response(socket, id)
  loop do
    message = read_message(socket, MESSAGE_TIMEOUT_SECONDS)
    return message if message["id"] == id

    next unless message["method"] && message.key?("id")

    send_message(
      socket,
      {
        "jsonrpc" => "2.0",
        "id" => message["id"],
        "result" => nil,
      },
    )
  end
end

binary, subject_path = ARGV
if !binary || !subject_path || ARGV.length != 2
  warn "usage: benchmark_lsp_client.rb BINARY SUBJECT_PATH"
  exit 2
end
abort "LSP binary not executable: #{binary}" unless File.executable?(binary)
abort "LSP subject not found: #{subject_path}" unless Dir.exist?(subject_path)

rb_files = Dir.glob(File.join(subject_path, "**", "*.rb"))
abort "no Ruby files found under #{subject_path}" if rb_files.empty?

target_file = rb_files.min_by do |path|
  normalized = path.tr("\\", "/")
  priority = if normalized.include?("/app/models/") && normalized.end_with?("/account.rb")
               0
             elsif normalized.include?("/app/models/")
               1
             elsif normalized.include?("/models/")
               2
             elsif normalized.include?("/app/controllers/")
               3
             else
               4
             end
  [priority, normalized.length, normalized]
end

root_path = File.basename(subject_path) == "app" ? File.dirname(subject_path) : subject_path
root_uri = file_uri(root_path)
target_uri = file_uri(target_file)
stdin = stdout = stderr = wait_thread = socket = stderr_thread = nil

begin
  stdin, stdout, stderr, wait_thread = Open3.popen3(binary, "--lsp")
  stdin.close
  stderr_thread = Thread.new { stderr.read }

  startup_line = Timeout.timeout(STARTUP_TIMEOUT_SECONDS) { stdout.gets }
  raise "LSP server did not publish startup information" unless startup_line

  startup = JSON.parse(startup_line)
  socket = TCPSocket.new(startup.fetch("host"), Integer(startup.fetch("port")))

  send_message(
    socket,
    {
      "jsonrpc" => "2.0",
      "id" => 1,
      "method" => "initialize",
      "params" => {
        "processId" => Process.pid,
        "clientInfo" => { "name" => "tyda-perf" },
        "rootUri" => root_uri,
        "capabilities" => {},
        "workspaceFolders" => [{ "uri" => root_uri, "name" => File.basename(root_path) }],
      },
    },
  )
  response = wait_for_response(socket, 1)
  raise "LSP initialize failed: #{response["error"]}" if response["error"]

  started = monotonic_seconds
  send_message(
    socket,
    {
      "jsonrpc" => "2.0",
      "id" => 2,
      "method" => "textDocument/hover",
      "params" => {
        "textDocument" => { "uri" => target_uri },
        "position" => { "line" => 0, "character" => 0 },
      },
    },
  )
  response = wait_for_response(socket, 2)
  raise "LSP hover failed: #{response["error"]}" if response["error"]

  elapsed_ms = ((monotonic_seconds - started) * 1000).round
  puts "[bench] workspace scan: #{elapsed_ms}ms (#{rb_files.length} files)"
rescue JSON::ParserError, KeyError, SocketError, SystemCallError, Timeout::Error => error
  warn error.message
  exit 1
ensure
  socket&.close
  if wait_thread&.alive?
    begin
      Process.kill("TERM", wait_thread.pid)
    rescue Errno::ESRCH
      nil
    end
    begin
      Timeout.timeout(5) { wait_thread.join }
    rescue Timeout::Error
      Process.kill("KILL", wait_thread.pid) rescue nil
      wait_thread.join
    end
  end
  [stdout, stderr].compact.each { |io| io.close unless io.closed? }
  stderr_thread&.join(1)
end
