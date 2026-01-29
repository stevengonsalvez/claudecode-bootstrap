class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  license "MIT"
  version "0.5.2-beta1"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.5.2-beta1/agents-box-0.5.2-beta1-aarch64-apple-darwin.tar.gz"
      sha256 "a9298bb522ae525d873774cdbaeb9ac2d170466c645ce998e628c1880f094145"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v0.5.2-beta1/agents-box-0.5.2-beta1-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "cd85cc6c5c942e6b343189c69098918f51d3ef5ae42792f451742b8e5de89ef8"
    end
  end

  def install
    bin.install "agents-box"
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
