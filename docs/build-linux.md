# Build the setup app on Linux (from source)

No prebuilt Linux installer is published. Compile the Tauri wizard on your machine,
or use the lighter **CLI-only** path in [`agent-on-rails-cli` `docs/linux-server.md`](https://github.com/agent-on-rails/agent-on-rails-cli/blob/main/docs/linux-server.md).

## Requirements

- Node.js **20+**
- Rust via [rustup](https://rustup.rs/) (stable)
- Python **3.11+** (the wizard installs/uses `aor`)
- System libraries for Tauri / WebKitGTK

### Debian / Ubuntu

```bash
sudo apt update
sudo apt install -y \
  curl build-essential pkg-config \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  libssl-dev
```

### Fedora

```bash
sudo dnf install -y \
  webkit2gtk4.1-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  openssl-devel
```

## Build

```bash
git clone https://github.com/agent-on-rails/agent-on-rails-desktop.git
cd agent-on-rails-desktop
npm install
npm run tauri build
```

Artifacts land under `src-tauri/target/release/bundle/` (e.g. `.deb`, AppImage, or binary depending on host).

Dev mode (no installer):

```bash
npm run tauri dev
```

## After it launches

Use the wizard to install `aor` and run `aor init`, then continue with the
[walkthrough](https://github.com/agent-on-rails/agent-on-rails-cli/blob/main/docs/walkthrough.md).

Prefer no GUI? Skip this app and install the CLI only:

```bash
pipx install git+https://github.com/agent-on-rails/agent-on-rails-cli.git
aor
```
