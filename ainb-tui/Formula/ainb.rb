class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  license "MIT"
  version "0.2.1"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.2.1/agents-box-0.2.1-aarch64-apple-darwin.tar.gz"
      sha256 ""
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.2.1/agents-box-0.2.1-x86_64-unknown-linux-gnu.tar.gz"
      sha256 ""
    end
  end

  def install
    bin.install "agents-box"
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
