# Building quickRS on a Mac

quickRS is open source (MIT). The source is at <https://github.com/arpan2711/quickRS-player>; each release page
also has it as *Source code (zip / tar.gz)*. This builds the same app as the release: a universal `quickRS.app`
for Apple silicon and Intel, macOS 11 or later.

## What you need (once)

```sh
xcode-select --install                       # Apple's Command Line Tools: compiler, libclang, codesign, lipo
curl https://sh.rustup.rs -sSf | sh          # Rust; then open a new Terminal window
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Full Xcode works too, but isn't needed. The repo's `rust-toolchain.toml` picks stable Rust (1.90 or later) and
installs rustfmt and clippy. Nothing from Homebrew is required.

## Get the source

```sh
git clone https://github.com/arpan2711/quickRS-player.git
cd quickRS-player
```

## Try it (your Mac's own architecture, quick)

```sh
export LIBCLANG_PATH="$(xcode-select -p)/usr/lib"    # bindgen (the time stretcher's C++ binding) needs libclang
cargo test --workspace                                # about 480 tests; a few minutes the first time
cargo run --release -p qrs-app                        # starts quickRS
```

`cargo run --release -p qrs-cli -- --help` shows the command-line tools (`devices` lists your audio inputs).

A bare binary run from Terminal borrows Terminal's microphone permission; the bundled app below asks for its own.

## Build the release (universal app, zip, dmg)

```sh
tools/mac/bundle.sh
```

It finds libclang by itself, builds both architectures, joins them with `lipo`, and makes in `dist/mac/`:

| File | What it is |
|---|---|
| `quickRS.app` | the app; `quickrs-cli` and `quickrs-convert` are inside `Contents/MacOS` |
| `quickRS-<ver>-macos-universal.zip` | the app zipped (the name the in-app updater looks for) |
| `quickRS-<ver>-macos-universal.dmg` | the same app, to drag into Applications (`NO_DMG=1` skips it) |
| `SHA256SUMS.txt` | checksums of the zip and the dmg |

It takes about 4 minutes on an M-series MacBook Air after the first build. `MACOSX_DEPLOYMENT_TARGET`
(default `11.0`) sets the oldest macOS supported.

The app is signed ad hoc (`codesign -s -`): fine on the Mac that built it. A copy downloaded from elsewhere is
quarantined, so its first open goes through System Settings > Privacy & Security > *Open Anyway* (see
README-macOS.txt). Notarizing needs a paid Apple developer account and isn't done.

## Install your build

```sh
ditto dist/mac/quickRS.app /Applications/quickRS.app
open /Applications/quickRS.app
```

Allow the microphone when asked: macOS needs it for any audio input, interfaces included. Each ad-hoc build is a
new app to macOS's privacy settings; if a rebuilt app hears nothing, reset its permission and open it again:

```sh
tccutil reset Microphone io.github.arpan2711.quickrs
```

Settings and keys are in `~/Library/Application Support/quickRS/`. Rename that folder to see the first-start
welcome and guitar setup again.

## Automatic builds

`.github/workflows/release-mac.yml` does the same on GitHub's macOS runner: for a pushed `v*` tag it runs the
tests, `tools/mac/bundle.sh`, and attaches the zip and dmg to that tag's release, adding their lines to the
release's `SHA256SUMS.txt`.

## Troubleshooting

- **`libclang.dylib not found`**: install the Command Line Tools (`xcode-select --install`), or set
  `LIBCLANG_PATH` to a folder holding `libclang.dylib`.
- **`can't find crate for std` for `x86_64-apple-darwin`**: `rustup target add x86_64-apple-darwin`.
- **The guitar is silent**: the microphone permission (above), then Settings (F2) > Audio input / output.
