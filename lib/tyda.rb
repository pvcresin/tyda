# frozen_string_literal: true

require_relative "tyda/version"

# Ruby packaging wrapper around the Tyda CLI / LSP binary (a Rust executable).
#
# The gem ships a precompiled `tyda` binary and exposes its path so the `tyda`
# executable shim (exe/tyda) and editor integrations can hand off to it. The
# inference engine itself carries no Ruby runtime dependency.
module Tyda
  class Error < StandardError; end

  # Absolute path to the bundled native `tyda` binary.
  #
  # An explicit `TYDA_BINARY` env var wins (useful during development against a
  # local `cargo build` output).
  #
  # An installed gem bundles a precompiled per-platform binary under `libexec/`,
  # staged there by the release pipeline (.github/workflows/release-gem.yml). In
  # a source checkout libexec/ is git-ignored and absent, so set TYDA_BINARY to a
  # locally built `tyda` and we raise a clear error otherwise.
  def self.executable
    env = ENV["TYDA_BINARY"]
    return env if env && !env.empty? && File.exist?(env)

    bundled = File.expand_path("../libexec/#{binary_name}", __dir__)
    return bundled if File.exist?(bundled)

    raise Error, "tyda binary not found (looked for #{bundled}); set " \
                 "TYDA_BINARY to a locally built `tyda` (e.g. target/release/tyda)"
  end

  def self.binary_name
    Gem.win_platform? ? "tyda.exe" : "tyda"
  end
end
