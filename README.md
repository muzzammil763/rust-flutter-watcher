# Flutter Watcher

A high-performance Rust-based file watcher that automatically triggers Flutter Hot Reload whenever relevant project files change.

No more pressing `r` in the terminal — just save your file and watch your app reload instantly.

---

## Features

- **Auto Hot Reload** — detects file changes and sends `r\n` to the Flutter process automatically
- **Attach Mode** — connect to an already-running `flutter run` (VS Code, Android Studio, another terminal) instead of starting a new one
- **Smart Filtering** — only watches `lib/`, `assets/`, and `pubspec.yaml`; ignores `build/`, `.dart_tool/`, `.git/`, etc.
- **Debouncing** — merges rapid file changes into a single reload (default 300ms)
- **Process Lifecycle** — spawns or attaches to Flutter, waits for the ready banner, handles graceful shutdown on Ctrl+C
- **Cross-Platform** — uses OS-native file watching APIs (FSEvents on macOS, inotify on Linux, ReadDirectoryChangesW on Windows)

---

## Installation

### macOS / Linux (Homebrew)

```bash
brew tap muzzammil763/flutter-watcher
brew install rust_flutter_watcher
```

After installation, the `flutter-watcher` command is available globally:

```bash
flutter-watcher --version
```

### Updating

```bash
brew update
brew upgrade rust_flutter_watcher
```

> Do **not** use `brew reinstall` to update — that reinstalls the same version. Use `brew upgrade` to get the latest.

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

### Standard mode — starts its own `flutter run`

Navigate to any Flutter project and run:

```bash
flutter-watcher
```

### Attach mode — connect to an already-running app

If `flutter run` is already open (in VS Code, Android Studio, or another terminal), use `--attach` so flutter-watcher doesn't start a second instance:

```bash
# Terminal 1 — start your app however you normally do
flutter run

# Terminal 2 — attach the watcher to it
flutter-watcher --attach
```

Now every file save triggers hot reload on the app you already have running.

---

### Options

| Flag | Description |
|------|-------------|
| `-p, --path <PATH>` | Path to the Flutter project directory (default: current directory) |
| `-d, --debounce <MS>` | Debounce duration in milliseconds (default: 300) |
| `-v, --verbose` | Enable verbose/debug logging |
| `-c, --config <PATH>` | Path to a custom config file |
| `-a, --attach` | Attach to an already-running Flutter app instead of starting a new one |
| `--device-id <ID>` | Device ID to target when attaching (optional) |

### Examples

```bash
# Run in current directory (starts flutter run)
flutter-watcher

# Attach to an already-running app
flutter-watcher --attach

# Attach to a specific device
flutter-watcher --attach --device-id emulator-5554

# Run in a specific project path
flutter-watcher --path ./my_flutter_app

# Debug mode with custom debounce
flutter-watcher --verbose --debounce 500

# Use custom config file
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

### Standard mode

```
flutter-watcher
    ↓
Launch flutter run
    ↓
Wait for "Flutter run key commands" banner
    ↓
Start file watcher on lib/, assets/, pubspec.yaml
    ↓
Detect file change → Filter → Debounce
    ↓
Send "r\n" to Flutter stdin → Hot Reload
```

### Attach mode (`--attach`)

```
flutter run  ← already running somewhere
    ↓
flutter-watcher --attach
    ↓
Run flutter attach (connects to running app)
    ↓
Start file watcher on lib/, assets/, pubspec.yaml
    ↓
Detect file change → Filter → Debounce
    ↓
Send "r\n" to Flutter stdin → Hot Reload
```

---

## Releasing a New Version (maintainers)

Each new binary release must have a new version tag. Uploading a new binary to an existing tag will break SHA verification for existing users.

```bash
# 1. Bump version in Cargo.toml and rebuild
cargo build --release

# 2. Compute SHA256 of each binary
shasum -a 256 flutter-watcher-darwin-amd64
shasum -a 256 flutter-watcher-darwin-arm64
shasum -a 256 flutter-watcher-linux-amd64

# 3. Create a new GitHub release with tag vX.Y.Z and upload binaries

# 4. Update homebrew/rust_flutter_watcher.rb:
#    - bump version "X.Y.Z"
#    - update URLs to /releases/download/vX.Y.Z/...
#    - paste the new SHA256 values

# 5. Commit and push — users then run: brew update && brew upgrade rust_flutter_watcher
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
│   ├── flutter.rs    # Flutter process spawn/attach + stdin control
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
- [x] Attach mode (connect to already-running Flutter app)
- [x] Multi-device support via `--device-id`
- [ ] Process crash recovery
- [ ] VM Service integration (structured reload without stdin)
- [ ] Terminal dashboard UI
- [ ] IDE plugin integration

---

## License

MIT

---

Built with Rust + Tokio + notify
