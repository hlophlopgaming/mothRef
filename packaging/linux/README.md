# Linux desktop integration

From the extracted release directory, install the binary, launcher, icon and board
file type for your account:

```sh
install -Dm755 mothref "$HOME/.local/bin/mothref"
install -Dm644 org.mothref.MothRef.desktop "${XDG_DATA_HOME:-$HOME/.local/share}/applications/org.mothref.MothRef.desktop"
install -Dm644 org.mothref.MothRef.png "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/512x512/apps/org.mothref.MothRef.png"
install -Dm644 org.mothref.MothRef.xml "${XDG_DATA_HOME:-$HOME/.local/share}/mime/packages/org.mothref.MothRef.xml"
update-mime-database "${XDG_DATA_HOME:-$HOME/.local/share}/mime"
update-desktop-database "${XDG_DATA_HOME:-$HOME/.local/share}/applications"
```

Ensure `$HOME/.local/bin` is in your session's `PATH`. Launch MothRef from the
application menu. The stable desktop ID lets Wayland find its icon. The alternative
in Settings changes the in-app mark and supported native window/Dock icons;
installed launcher and file icons use the default moth.
