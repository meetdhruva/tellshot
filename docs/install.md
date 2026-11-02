
## Install

Pick the file for your system from the release assets.

### Windows 10 (1809) or later

Run `tellshot-<version>-x64-setup.exe` (or `arm64` on ARM devices). It installs for your user
account without administrator rights, adds Tellshot to the Start menu, can add `tellshot` to your
PATH, and opens `.tellshot` projects. Prefer no installer? Use the `.zip`.

The installer is not code-signed yet, so SmartScreen may show "Windows protected your PC": choose
**More info → Run anyway**.

### macOS 11 or later

Open `Tellshot-<version>-aarch64.dmg` (Apple silicon) or `x86_64.dmg` (Intel) and drag Tellshot to
Applications. The app is not notarized yet: the first time, right-click Tellshot in Applications and
choose **Open**, or run `xattr -dr com.apple.quarantine /Applications/Tellshot.app`.

To use the command-line tool:
`sudo ln -s /Applications/Tellshot.app/Contents/MacOS/tellshot /usr/local/bin/tellshot`.
Capturing the screen asks for Screen Recording permission in System Settings → Privacy & Security.

### Linux (x64 or ARM64, glibc 2.35+: Ubuntu 22.04, Debian 12, Fedora 36 or newer)

Debian and Ubuntu: `sudo apt install ./tellshot_<version>_amd64.deb` (or `arm64`). This adds the
app to your menu, registers `.tellshot` files and installs shell completions.

Other distributions: unpack the `.tar.gz` and put `tellshot` and `tellshot-editor` on your PATH.
Screen capture needs an X11 session for now; Wayland support is on the roadmap.

### Verify a download

```sh
sha256sum -c SHA256SUMS.txt --ignore-missing
gh attestation verify <file> --repo meetdhruva/tellshot
```
