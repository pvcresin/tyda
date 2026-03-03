# frozen_string_literal: true

module Tyda
  # Single source of version truth is `tyda-version.txt`
  # (`MAJOR.MINOR.YYYYMMDDHHMMSS`): humans bump the `MAJOR.MINOR` prefix; the
  # patch is a release timestamp injected by CI. Cutting a release is just
  # merging to main — the release pipeline sets TYDA_RELEASE_VERSION to
  # `MAJOR.MINOR.<commit timestamp>` and stamps the same value into both the gem
  # and the binary. Builds without that env (local dev, PR CI) get a dev
  # prerelease. The resolved value is baked into the gem at build time, so the
  # installed gem never needs `tyda-version.txt` at runtime.
  VERSION =
    if (release = ENV["TYDA_RELEASE_VERSION"]) && !release.strip.empty?
      release.strip
    else
      version_file = File.expand_path("../../tyda-version.txt", __dir__)
      prefix =
        if File.exist?(version_file)
          File.read(version_file).strip.split(".").first(2).join(".")
        else
          "0.0"
        end
      "#{prefix}.0.dev"
    end
end
