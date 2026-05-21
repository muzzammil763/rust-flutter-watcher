class RustFlutterWatcher < Formula
  desc "High-performance Rust-based file watcher for Flutter Hot Reload"
  homepage "https://github.com/muzzammil763/rust-flutter-watcher"
  version "0.2.5"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/muzzammil763/rust-flutter-watcher/releases/download/v0.2.5/flutter-watcher-darwin-arm64"
      sha256 "PLACEHOLDER_SHA256_ARM64"
    else
      url "https://github.com/muzzammil763/rust-flutter-watcher/releases/download/v0.2.5/flutter-watcher-darwin-amd64"
      sha256 "27aebbb841325f5ef001f9f985435234c10a21c3bbff893434960e3b6641d9b2"
    end
  elsif OS.linux?
    url "https://github.com/muzzammil763/rust-flutter-watcher/releases/download/v0.2.5/flutter-watcher-linux-amd64"
    sha256 "PLACEHOLDER_SHA256_LINUX"
  end

  def install
    if OS.mac? && Hardware::CPU.arm?
      bin.install "flutter-watcher-darwin-arm64" => "flutter-watcher"
    elsif OS.mac? && Hardware::CPU.intel?
      bin.install "flutter-watcher-darwin-amd64" => "flutter-watcher"
    elsif OS.linux?
      bin.install "flutter-watcher-linux-amd64" => "flutter-watcher"
    end
  end

  test do
    system "#{bin}/flutter-watcher", "--version"
  end
end
