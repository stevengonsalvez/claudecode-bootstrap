class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  license "MIT"
  version "0.0.0-beta1"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.0.0-beta1/agents-box-0.0.0-beta1-aarch64-apple-darwin.tar.gz"
      sha256 "2a1fe628628c6ce2b040b9265b84f9cd7cb1b3d74cfd4564b997bf6310dcffcf"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.0.0-beta1/agents-box-0.0.0-beta1-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "673c4c0b73b4262b74c62a77d85b0e6af75201a7ba4fe062cc3bfb3d408924f4"
    end
  end

  def install
    bin.install "agents-box"
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
