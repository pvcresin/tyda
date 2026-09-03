# frozen_string_literal: true

# Bundler is used only to fetch stdlib RBS type defs (core/stdlib) from the rbs
# gem. scripts/vendor-rbs.sh expands them into vendor/rbs/ for the inference
# engine. Ruby is not required at runtime.
#
# The C parser is vendored/compiled by the ruby-rbs-sys crate (via crates/rbs-sys);
# this Gemfile only pulls type-definition data. Dependabot (bundler) bumps
# Gemfile.lock for version updates.
source "https://rubygems.org"

gem "rbs", "= 4.1.3"
