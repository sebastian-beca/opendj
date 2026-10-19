#!/usr/bin/env bash
# Builds opendj.app into dist/: the landing pages as they are, plus the
# mdBook docs in dist/docs. Cloudflare Pages runs this with the project's
# root directory set to website/ and the output directory set to dist.
set -euo pipefail
cd "$(dirname "$0")"

MDBOOK_VERSION=0.4.52

rm -rf dist
mkdir -p dist
cp -R index.html style.css main.js _headers assets tutorials dist/

if ! command -v mdbook >/dev/null 2>&1; then
  case "$(uname -s)" in
    Linux) target=x86_64-unknown-linux-gnu ;;
    Darwin) target=$( [ "$(uname -m)" = arm64 ] && echo aarch64-apple-darwin || echo x86_64-apple-darwin ) ;;
    *) echo "Install mdbook $MDBOOK_VERSION first" >&2; exit 1 ;;
  esac
  mkdir -p .bin
  curl -sSfL "https://github.com/rust-lang/mdBook/releases/download/v$MDBOOK_VERSION/mdbook-v$MDBOOK_VERSION-$target.tar.gz" | tar -xz -C .bin
  export PATH="$PWD/.bin:$PATH"
fi

mdbook build book
echo "Built $(find dist -type f | wc -l) files into website/dist"
