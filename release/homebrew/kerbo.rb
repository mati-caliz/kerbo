class Kerbo < Formula
  desc "Analizador de calidad multi-lenguaje"
  homepage "https://kerbo.matiascaliz.com.ar"
  version "0.6.0"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/mati-caliz/kerbo/releases/download/v#{version}/kerbo-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
    on_intel do
      url "https://github.com/mati-caliz/kerbo/releases/download/v#{version}/kerbo-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/mati-caliz/kerbo/releases/download/v#{version}/kerbo-#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
    on_intel do
      url "https://github.com/mati-caliz/kerbo/releases/download/v#{version}/kerbo-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    bin.install "kerbo"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/kerbo --version")
  end
end
