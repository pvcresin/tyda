#!/usr/bin/env bash
set -euo pipefail

pages_dir="pages-dist"

rm -rf "$pages_dir"
mkdir -p "$pages_dir/play"
cp -R docs/.vitepress/dist/. "$pages_dir/"
cp -R playground/dist/. "$pages_dir/play/"

test -f "$pages_dir/index.html"
test -f "$pages_dir/docs/index.html"
test -f "$pages_dir/play/index.html"
