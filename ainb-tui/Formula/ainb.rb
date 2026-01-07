class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  version "0.1.0"
  license "MIT"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v#{version}/agents-box-#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "518f708ccf27d02e0146dc97c4a511f287f5dba41c760fd6ca2075609bbd3754"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v#{version}/agents-box-#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "9e41002c1836dc7992d3c4c94e13b328068725346c2c8b62fa2e7a392270317a"
  else
    # Build from source for Intel Mac and ARM Linux
    url "https://github.com/stevengonsalvez/agents-in-a-box.git", branch: "v2"
    depends_on "rust" => :build
  end

  def install
    if File.exist?("agents-box")
      # Pre-built binary - install and rename to 'ainb'
      bin.install "agents-box" => "ainb"
    else
      # Build from source
      cd "ainb-tui" do
        system "cargo", "build", "--release"
        bin.install "target/release/agents-box" => "ainb"
      end
    end
  end

  test do
    assert_match "agents-box", shell_output("#{bin}/ainb --version")
  end
end
