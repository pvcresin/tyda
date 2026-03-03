# frozen_string_literal: true

require_relative "lib/tyda/version"

Gem::Specification.new do |spec|
  # "tyda" is a working codename (see README); confirm it is free on
  # rubygems.org before the first publish.
  spec.name = "tyda"
  spec.version = Tyda::VERSION

  spec.authors = ["pvcresin"]
  spec.email = ["pvcresin0730@gmail.com"]
  spec.summary = "Fast Ruby/Rails type inference engine: RBS output CLI + TypeProf-compatible LSP."
  spec.description = spec.summary
  spec.homepage = "https://github.com/pvcresin/tyda"
  spec.license = "MIT"

  spec.required_ruby_version = ">= 3.0"

  spec.metadata["homepage_uri"] = spec.homepage
  spec.metadata["source_code_uri"] = spec.homepage
  spec.metadata["changelog_uri"] = "#{spec.homepage}/releases"
  # SAFETY guard against an accidental publish while the gem is not ready:
  # rubygems.org refuses a push whose allowed_push_host does not match it. Flip
  # this to "https://rubygems.org" (one line) when ready to release.
  spec.metadata["allowed_push_host"] = "https://rubygems.example.invalid"

  # Ship the Ruby wrapper plus the precompiled binary and its stdlib RBS data,
  # staged into libexec/ by the release pipeline (libexec/<bin> +
  # libexec/vendor/rbs). Deliberately NOT the Rust sources.
  spec.files = Dir["lib/**/*.rb", "exe/*", "libexec/**/*", "LICENSE", "THIRD-PARTY-NOTICES.md", "README.md"]
  spec.bindir = "exe"
  spec.executables = ["tyda"]
  spec.require_paths = ["lib"]

  # When a native binary is staged this is a platform-specific gem (the binary
  # only runs on its build target). Without it (gemspec validation / wrapper
  # build) it stays a generic Ruby gem. The release CI sets TYDA_GEM_PLATFORM to
  # a version-agnostic string (e.g. arm64-darwin or x64-mingw-ucrt, not
  # arm64-darwin-24); locally it falls back to the current platform.
  if File.exist?(File.expand_path("libexec/tyda", __dir__)) ||
     File.exist?(File.expand_path("libexec/tyda.exe", __dir__))
    spec.platform = ENV.fetch("TYDA_GEM_PLATFORM") { Gem::Platform.local.to_s }
  end
end
