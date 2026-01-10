class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  license "MIT"
  version "0.3.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.3.0/agents-box-0.3.0-aarch64-apple-darwin.tar.gz"
      sha256 "0420b9ec981d32d5e2c26ffe5dd31db9c2608f849b403f1df6017c214add4cd0"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.3.0/agents-box-0.3.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "b93a30d30f14914c19939f9b37d78c263ddbe0d647b050557fe3b6f45e048a83"
    end
  end

  def install
    bin.install "agents-box"
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
