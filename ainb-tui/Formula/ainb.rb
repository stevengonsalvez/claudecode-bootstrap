class Ainb < Formula
  desc "Terminal-based development environment manager for Claude Code agents"
  homepage "https://github.com/stevengonsalvez/agents-in-a-box"
  version "0.2.0"
  license "MIT"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/stevengonsalvez/agents-in-a-box/releases/download/v#{version}/agents-box-#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "b744516edb17179a731de59bfe3e9b15e26bb87674ae51138a45b95a4c08d91f"
  else
    # Build from source for Intel Mac, Linux, etc.
    url "https://github.com/stevengonsalvez/agents-in-a-box.git", tag: "v#{version}"
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
    assert_match "Terminal-based development environment", shell_output("#{bin}/ainb --help")
  end
end
