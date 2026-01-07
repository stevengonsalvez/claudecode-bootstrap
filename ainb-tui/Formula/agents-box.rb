class AgentsBox < Formula
  desc "Terminal-based development environment manager for Claude Code"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v#{version}/agents-box-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "518f708ccf27d02e0146dc97c4a511f287f5dba41c760fd6ca2075609bbd3754"
    end
    on_intel do
      # Intel Mac: build from source
      depends_on "rust" => :build
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v#{version}/agents-box-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      # Add Linux checksum here
    end
  end

  def install
    if build.bottle? || (OS.mac? && Hardware::CPU.arm?)
      bin.install "agents-box"
    else
      # Build from source for Intel Mac
      system "cargo", "install", *std_cargo_args(path: "ainb-tui")
    end
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/agents-box --version")
  end
end
