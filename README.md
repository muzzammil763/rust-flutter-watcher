# Flutter Watcher

A high-performance Rust-based file watcher that automatically triggers Flutter Hot Reload whenever relevant project files change.

No more pressing `r` in the terminal — just save your file and watch your app reload instantly.

---

## Features

- **Auto Hot Reload** — detects file changes and sends `r\n` to the running Flutter process automatically
- **Smart Filtering** — only watches `lib/`, `assets/`, and `pubspec.yaml`; ignores `build/`, `.dart_tool/`, `.git/`, etc.
- **Debouncing** — merges rapid file changes into a single reload (default 300ms)
- **Process Lifecycle** — spawns `flutter run`, waits for the ready banner, handles graceful shutdown on Ctrl+C
- **Cross-Platform** — uses OS-native file watching APIs (FSEvents on macOS, inotify on Linux, ReadDirectoryChangesW on Windows)

---

## Installation

### macOS (Homebrew)

```bash
brew tap muzzammil763/flutter-watcher
brew install rust_flutter_watcher
```

After installation, the `flutter-watcher` command is available globally:

```bash
flutter-watcher --version
```

### Build from Source

Requires [Rust](https://rustup.rs/) (1.70+).

```bash
git clone https://github.com/muzzammil763/rust-flutter-watcher.git
cd rust-flutter-watcher
cargo build --release
```

The binary will be at `target/release/flutter-watcher`. You can move it to your PATH:

```bash
cp target/release/flutter-watcher /usr/local/bin/
```

---

## Usage

Navigate to any Flutter project and run:

```bash
flutter-watcher
```

### Options

| Flag | Description |
|------|-------------|
| `-p, --path <PATH>` | Path to the Flutter project directory (default: current directory) |
| `-d, --debounce <MS>` | Debounce duration in milliseconds (default: 300) |
| `-v, --verbose` | Enable verbose/debug logging |
| `-c, --config <PATH>` | Path to a custom config file |

### Examples

```bash
# Run in current directory
flutter-watcher

# Run in a specific project
flutter-watcher --path ./my_flutter_app

# Debug mode with custom debounce
flutter-watcher --verbose --debounce 500

# Use custom config
flutter-watcher --config ./custom-watcher.toml
```

---

## Config File

Place a `flutter-watcher.toml` in your project root or pass it with `--config`:

```toml
watch_paths = ["lib", "assets", "pubspec.yaml"]

ignore_paths = [
  "build",
  ".dart_tool",
  ".git",
  "ios/Pods",
  "android/build",
  "android/.gradle"
]

file_extensions = [
  "dart",
  "yaml",
  "yml",
  "json",
  "png",
  "jpg",
  "jpeg",
  "svg",
  "webp",
  "gif"
]

debounce_ms = 300
```

---

## How It Works

```
Start flutter-watcher
    ↓
Launch flutter run
    ↓
Wait for "Flutter run key commands" banner
    ↓
Start file watcher on lib/, assets/, pubspec.yaml
    ↓
Detect file change → Filter → Debounce
    ↓
Send "r\n" to Flutter stdin
    ↓
Flutter Hot Reload triggers automatically
```

---

## Requirements

- Flutter SDK installed and in your PATH
- macOS, Linux, or Windows

---

## Project Structure

```
rust-flutter-watcher/
├── src/
│   ├── main.rs       # CLI entry + async event loop
│   ├── cli.rs        # Argument parsing (clap)
│   ├── config.rs     # TOML config loader + filtering rules
│   ├── events.rs     # File event types
│   ├── flutter.rs    # Flutter process spawn + stdin control
│   └── watcher.rs    # notify-based file watcher
├── Cargo.toml
├── README.md
├── flutter-watcher.toml
└── homebrew/
    └── rust_flutter_watcher.rb
```

---

## Roadmap

- [x] MVP: spawn flutter, watch files, debounce, send reload
- [x] Config file support
- [x] CLI arguments
- [ ] Process crash recovery
- [ ] VM Service integration (structured reload)
- [ ] Multi-device support
- [ ] Terminal dashboard UI
- [ ] IDE plugin integration

---

## License

MIT

---

Built with Rust + Tokio + notify
