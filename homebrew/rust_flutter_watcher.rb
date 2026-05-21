class RustFlutterWatcher < Formula
  desc "High-performance Rust-based file watcher for Flutter Hot Reload"
  homepage "https://github.com/muzzammil763/rust-flutter-watcher"
  version "0.1.0"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/muzzammil763/rust-flutter-watcher/releases/download/v0.1.0/flutter-watcher-darwin-arm64"
      sha256 "PLACEHOLDER_SHA256_ARM64"
    else
      url "https://github.com/muzzammil763/rust-flutter-watcher/releases/download/v0.1.0/flutter-watcher-darwin-amd64"
      sha256 "ef2e0f6234b3bcc41e0a76174d81d3516d50ab580f6c7ce7b39fc02f29c65611"
    end
  elsif OS.linux?
    url "https://github.com/muzzammil763/rust-flutter-watcher/releases/download/v0.1.0/flutter-watcher-linux-amd64"
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
    system "#{bin}/flutter-watcher", "--help"
  end
end
