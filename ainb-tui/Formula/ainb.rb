class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  license "MIT"
  version "0.4.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.4.0/agents-box-0.4.0-aarch64-apple-darwin.tar.gz"
      sha256 "6b1168a646da625d7553c5eeb7ee7ba475f1fd2a5df8d7b54cb8162851baf2be"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.4.0/agents-box-0.4.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "64647f59b6e3ce151817678efd6a370625c35b4145926a951c6e8f5a3fe11ef6"
    end
  end

  def install
    bin.install "agents-box"
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
