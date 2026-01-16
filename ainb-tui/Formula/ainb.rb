class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  license "MIT"
  version "0.5.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.5.0/agents-box-0.5.0-aarch64-apple-darwin.tar.gz"
      sha256 "cde8797e5ac2d3c0f4e9e8d750eeedb9f927fa0084f0b90bf3b73a37f7d4f718"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.5.0/agents-box-0.5.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "f42017196d9cbd35b2143129ca2f78d2b3afa9542b7bafcf2a2852d67496a013"
    end
  end

  def install
    bin.install "agents-box"
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
