class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  license "MIT"
  version "0.5.1-beta1"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.5.1-beta1/agents-box-0.5.1-beta1-aarch64-apple-darwin.tar.gz"
      sha256 "97a66ba2e0b2f8ee1756e0e47f03de022ac5efb8258a2451a0027106e94c4f9f"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.5.1-beta1/agents-box-0.5.1-beta1-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "64b8161b055a59a131d4f70fa07408e5a922e05e9afdfbe2dd8eee845c32caa3"
    end
  end

  def install
    bin.install "agents-box"
    bin.install_symlink "agents-box" => "ainb"
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
