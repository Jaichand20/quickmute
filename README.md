# QuickMute 🎙️🔇

An ultra-lightweight, zero-bloat Windows microphone muting utility written in pure **Rust**.

Designed specifically for gamers, streamers, and callers who want an instant hardware-style mute button on their keyboard without running heavy bloatware suites.

## Features

- **Blazing Fast**: Native Win32 (RegisterHotKey) and Windows CoreAudio COM APIs (IAudioEndpointVolume).
- **Tiny Footprint**: Standalone native .exe is only **~108 KB** with **< 5 MB RAM** and **0% CPU** idle usage.
- **Dedicated Global Hotkey**: Uses your physical **Home** key (virtual key code VK_HOME) to toggle mute system-wide. Works even while playing fullscreen exclusive games!
- **System Tray Status**: Sits cleanly in your Windows system tray:
  - 🟢 **Green Icon**: Microphone is Live / Active.
  - 🔴 **Red Icon**: Microphone is Muted.
- **Audio Tone Feedback**: Plays a distinct audio chime when muting / unmuting so you know your status without looking away from your game.
- **Universal OS Mute**: Directly mutes the Windows default communications device (e.g. Focusrite Scarlett, USB mic, etc.), instantly muting Discord, in-game voice chat (CS2, Valorant, Call of Duty), Zoom, and Teams simultaneously.

## How to Run

1. Download or build quickmute.exe.
2. Run quickmute.exe. It will start silently in your system tray.
3. Tap your physical **Home** key anytime to toggle mute.
4. Right-click the tray icon to manually toggle or click **Exit QuickMute**.

## Building from Source

Requirements: [Rust toolchain](https://rustup.rs/) (edition 2021)

`ash
git clone https://github.com/Jaichand20/quickmute.git
cd quickmute
cargo build --release
`

The compiled binary will be in 	arget/release/quickmute.exe.

## License

MIT License
