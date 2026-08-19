#!/usr/bin/env python3
"""Regenerate Formula/wless.rb in the homebrew-wless tap for a release tag.

Usage: update_homebrew_formula.py <tag> <output-path>

<tag> is a git tag like "v1.4.0". <output-path> is where to write the
generated formula (e.g. tap/Formula/wless.rb).

Downloads each platform's release tarball and hashes it locally rather
than trusting a precomputed checksum, so the formula's sha256 always
matches exactly what's actually being served.
"""

import hashlib
import sys
import urllib.request

ASSETS = [
    ("aarch64-apple-darwin", "on_macos", "on_arm"),
    ("x86_64-apple-darwin", "on_macos", "on_intel"),
    ("aarch64-unknown-linux-gnu", "on_linux", "on_arm"),
    ("x86_64-unknown-linux-gnu", "on_linux", "on_intel"),
]

FORMULA_TEMPLATE = '''class Wless < Formula
  desc "Word-wrapping, auto-following terminal pager"
  homepage "https://github.com/phurley/wless"
  version "{version}"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/phurley/wless/releases/download/{tag}/wless-{tag}-aarch64-apple-darwin.tar.gz"
      sha256 "{sha_aarch64_apple_darwin}"
    end
    on_intel do
      url "https://github.com/phurley/wless/releases/download/{tag}/wless-{tag}-x86_64-apple-darwin.tar.gz"
      sha256 "{sha_x86_64_apple_darwin}"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/phurley/wless/releases/download/{tag}/wless-{tag}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "{sha_aarch64_unknown_linux_gnu}"
    end
    on_intel do
      url "https://github.com/phurley/wless/releases/download/{tag}/wless-{tag}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "{sha_x86_64_unknown_linux_gnu}"
    end
  end

  def install
    bin.install "wless"
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/wless --version")
  end
end
'''


def sha256_of_url(url: str) -> str:
    digest = hashlib.sha256()
    with urllib.request.urlopen(url) as resp:
        while chunk := resp.read(1 << 16):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <tag> <output-path>", file=sys.stderr)
        sys.exit(1)
    tag, output_path = sys.argv[1], sys.argv[2]
    version = tag.removeprefix("v")

    shas = {}
    for target, _, _ in ASSETS:
        url = f"https://github.com/phurley/wless/releases/download/{tag}/wless-{tag}-{target}.tar.gz"
        print(f"Hashing {url} ...")
        shas[target.replace("-", "_")] = sha256_of_url(url)

    formula = FORMULA_TEMPLATE.format(
        version=version,
        tag=tag,
        sha_aarch64_apple_darwin=shas["aarch64_apple_darwin"],
        sha_x86_64_apple_darwin=shas["x86_64_apple_darwin"],
        sha_aarch64_unknown_linux_gnu=shas["aarch64_unknown_linux_gnu"],
        sha_x86_64_unknown_linux_gnu=shas["x86_64_unknown_linux_gnu"],
    )

    with open(output_path, "w") as f:
        f.write(formula)
    print(f"Wrote {output_path}")


if __name__ == "__main__":
    main()
