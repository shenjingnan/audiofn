<div align="right">

[简体中文](README.md) | **English**

</div>

<div align="center">
  <img src="docs/public/logo.svg" alt="AudioFn Logo" width="300" />

  <p>
    <a href="https://github.com/shenjingnan/audiofn/releases"><img src="https://img.shields.io/github/v/release/shenjingnan/audiofn" alt="GitHub Release" /></a>
    <a href="https://crates.io/crates/audiofn"><img src="https://img.shields.io/crates/v/audiofn" alt="crates.io version" /></a>
    <a href="https://crates.io/crates/audiofn"><img src="https://img.shields.io/crates/d/audiofn" alt="crates.io downloads" /></a>
    <a href="https://github.com/shenjingnan/audiofn/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/shenjingnan/audiofn/ci.yml?branch=main&label=CI" alt="GitHub Actions CI status" /></a>
    <a href="https://codecov.io/gh/shenjingnan/audiofn"><img src="https://codecov.io/gh/shenjingnan/audiofn/graph/badge.svg" alt="Codecov coverage" /></a>
    <br />
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0--only-blue" alt="License: GPL-3.0-only" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.97%2B-dea584?logo=rust" alt="Rust 1.97+" /></a>
    <a href="#app-download"><img src="https://img.shields.io/badge/macOS-000000?logo=apple&logoColor=white" alt="macOS support" /></a>
    <a href="#app-download"><img src="https://img.shields.io/badge/Linux-FCC624?logo=linux&logoColor=black" alt="Linux support" /></a>
  </p>
</div>

**AudioFn** — An open-source, local-first desktop ASR & TTS toolkit with voice cloning.

Offline speech recognition and voice cloning that run entirely on your device. No audio or text ever leaves your machine.

## ✨ Features

- **Speech recognition (ASR)** — Offline transcription with Qwen3-ASR-0.6B, automatic language detection across 30 languages; available from the `audiofn asr` CLI and the desktop transcription page (file transcription / dictation)
- **Text-to-speech (TTS)** — Zero-shot voice cloning with Qwen3-TTS-0.6B / 1.7B: record 3–10 seconds (5–10 recommended) of reference audio plus its transcript, then synthesize any text in that voice (10 languages including Chinese, English, Japanese and Korean)
- **Desktop + CLI** — A Tauri 2 desktop panel (Overview / Model library / Transcribe / Synthesize / Voice library / Settings) alongside the `audiofn asr` / `audiofn tts` command line
- **Local-first** — Powered by the audio.cpp sidecar engine (ggml; Metal on macOS, CPU on Linux). Models are one click away, and nothing is uploaded
- **Model library** — One-click download, sha256 verification and model switching from the desktop "Models" page; model binaries are never committed, only their manifest (source / checksum / license) is versioned
- **Platforms** — macOS 13+ (Apple Silicon / Intel) and Linux x86_64 (deb / rpm / AppImage); **no Windows builds**

## App Download

Click a button below to grab the latest installer for your system (no GitHub login required; always points to the latest release):

| OS | Chip / Arch | Download |
| --- | --- | --- |
| macOS 13+ | Apple Silicon (M1/M2/M3/M4) | [![Download](https://img.shields.io/badge/Download-8E8E93?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_macOS_arm64.dmg) |
| macOS 13+ | Intel | [![Download](https://img.shields.io/badge/Download-8E8E93?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_macOS_x64.dmg) |
| Ubuntu / Debian | amd64 | [![Download](https://img.shields.io/badge/Download-A80030?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_Linux_amd64.deb) |
| Fedora / RHEL | x86_64 | [![Download](https://img.shields.io/badge/Download-294172?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_Linux_x86_64.rpm) |

- On Linux you can also grab the [AppImage](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_Linux_amd64.AppImage) and run it without installing.
- See [Releases](https://github.com/shenjingnan/audiofn/releases) for the full version history and changelogs.
- 🍎 Not sure which Mac chip you have? Click the  menu → "About This Mac": "Chip: Apple M…" means arm64, "Processor: Intel…" means x64.
- 📦 Models are not bundled with the installers: download them from the in-app "Models" page on first run (ASR ≈ 1.1 GB, TTS from ≈ 1.9 GB).

### First launch on macOS (unsigned)

The project does not hold an Apple Developer certificate, so the installers are **unsigned**. Gatekeeper blocks the first launch with "AudioFn" is damaged and can't be opened — the app is **not** actually damaged; the system just added a quarantine attribute to the downloaded file. Two ways to fix it:

- **Run the bundled fixer (recommended)**: open the downloaded dmg and double-click **首次打开修复.command** (First-launch fixer). It installs AudioFn into Applications, clears the quarantine attribute and launches the app. If macOS says the file "cannot verify the developer", right-click it → "Open" → "Open" again.
- **Run the command manually**: drag AudioFn into Applications, then open Terminal and run:

  ```bash
  xattr -cr "/Applications/AudioFn.app"
  ```

If the app is not in Applications, replace the path with the actual location; or right-click the app → "Open" → "Open" again.

## Quick Start (CLI)

```bash
# Download models (models live outside the repo, installed to ~/.audiofn/models/)
cargo run -- asr install-model                     # Download the ASR model (qwen3-asr-0.6b)
cargo run -- tts install-model                     # Download the TTS model (qwen3-tts-0.6b)

# Speech recognition
cargo run -- asr transcribe --wav rec.wav          # Transcribe a file (language auto-detected)
cargo run -- asr dictate                           # Dictate (press Enter or Ctrl-C to stop)
cargo run -- asr devices                           # List input devices

# Speech synthesis (voice cloning)
cargo run -- tts run --text "Hello" --reference-wav ref.wav --reference-text "reference transcript" --output out.wav
cargo run -- tts voices                            # Voice library (custom voices; cloning required)

# Misc
cargo run -- config                                # Show config
cargo run -- completion bash                       # Shell completions (zsh / fish / powershell / elvish likewise)
```

Voice cloning in three steps: record 3–10 seconds (5–10 recommended) of clean reference audio → write down exactly what it says → pass both as `--reference-wav` + `--reference-text` to `tts run`. You can also record voices in the desktop "Voice library" and reuse them later with `--voice <voice-id>`.

## Development

```bash
# Development
cargo run                          # Run without arguments to see help
cargo build                        # Debug build
cargo test                         # Run tests

# Code quality (full check, same as CI)
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

- [Contributing guide](CONTRIBUTING.md): environment setup, CLI & configuration reference, the audio.cpp engine, project structure and the release process
- [Documentation site](docs/): introduction, desktop app, CLI, model library, voice cloning guide and FAQ (mostly in Chinese)
- Desktop app development: `pnpm install` → `scripts/fetch-audiocpp-dev.sh` (places the engine sidecar) → `pnpm tauri dev`

## Project Structure

```
├── Cargo.toml           # workspace root (crate: audiofn, CLI bin: audiofn-cli)
├── rust-toolchain.toml  # pinned toolchain 1.97.1
├── src/
│   ├── main.rs          # entry point
│   ├── lib.rs           # library root + test helpers (test_util HOME isolation)
│   ├── cli.rs           # CLI definition (asr / tts / config / completion)
│   ├── asr/             # speech recognition (Qwen3-ASR: offline transcription + dictation)
│   ├── tts/             # speech synthesis (Qwen3-TTS: synthesis + voices + voice library)
│   ├── audiocpp/        # audio.cpp sidecar client (locate / lifecycle / SSE parsing)
│   ├── model_library/   # model library (registry / download / sha256 / install / switch)
│   ├── audio.rs         # cpal microphone capture + resampling
│   ├── config/          # settings.toml + shortcuts
│   ├── logging.rs       # tracing dual-layer logging (file + stderr)
│   └── datetime.rs      # date/time helpers
├── models/              # model manifests (source / sha256 / license; binaries not committed)
├── src-tauri/           # Tauri 2 desktop app (workspace member)
│   ├── src/lib.rs       # Tauri commands + dictate / synthesis threads
│   ├── frontend/        # React + Vite + TypeScript panel (Tailwind + shadcn/ui)
│   ├── tauri.conf.json  # Tauri config (bundle / externalBin / permissions)
│   ├── capabilities/    # capability declarations
│   └── icons/           # app icons
├── docs/                # fumadocs documentation site (Next.js + MDX)
├── tests/               # integration tests
├── scripts/             # engine fetch / dmg fixer injection / icon generation
├── .github/             # CI / release / cloud-drive upload workflows
└── .githooks/           # Git hooks
```

## Known Limitations

- **No VAD / long-audio splitting in phase one**: dictation and file transcription send the whole clip to the model; hour-long audio will use a lot of memory, so split it yourself first.
- **TTS is whole-utterance, non-streaming**: each request synthesizes the full text at once, with no playback while generating; long texts take proportionally longer.
- **Linux is CPU-only**: the Linux engine build has no GPU backend enabled; inference runs on the CPU. macOS (Apple Silicon) uses Metal.
- **No Windows builds**: the build and installer whitelist covers macOS and Linux only.

## License

[GPL-3.0-only](LICENSE)
