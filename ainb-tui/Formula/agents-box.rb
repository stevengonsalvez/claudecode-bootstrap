class AgentsBox < Formula
  desc "Terminal-based development environment manager for Claude Code"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  version "0.1.0"
  license "MIT"

  # For local development, use:
  # url "file:///path/to/agents-box-0.1.0.tar.gz"

  # For releases, use:
  # url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v#{version}/agents-box-#{version}-darwin-arm64.tar.gz"
  # sha256 "CHECKSUM_HERE"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
