<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="MothRef-Logo: eine Motte in Pixel-Art" width="128" height="128">
</picture>

# MothRef

**Ein schnelles, absturzsicheres Referenzboard für Künstler.**<br>
Bilder auf eine unendliche Leinwand ziehen, anordnen und beim Arbeiten im Blick behalten.

[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Download**](#download) · [Funktionen](#features) · [Bedienung](#getting-around) · [Roadmap](#roadmap) · [Selbst bauen](#build-from-source) · [Dokumentation](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <b>Deutsch</b> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="Ein MothRef-Board mit drei Illustrationen auf dunkler Leinwand" width="900">

</div>

> [!WARNING]
> MothRef ist in einer **frühen Alpha** (`0.1.0-alpha`). Speichern, Wiederherstellen und Rückgängig funktionieren, aber lange Zuverlässigkeitstests stehen noch aus. Bewahre separate Kopien wichtiger Boards und der Originalbilder auf.

---

<a id="why-mothref"></a>

## Warum MothRef

[BeeRef](https://github.com/rbreu/beeref) war ein wunderbares quelloffenes Referenzboard, wird aber nicht mehr weiterentwickelt. [PureRef](https://www.pureref.com) ist schnell und ausgereift, aber proprietär. MothRef macht dort weiter, wo BeeRef aufgehört hat: nativ, offen und für riesige Boards gebaut, ohne den Arbeitsspeicher aufzufressen.

Es beruht auf drei Versprechen, in dieser Reihenfolge:

| Versprechen | Was das in der Praxis bedeutet |
| --- | --- |
| **Deine Arbeit geht nie verloren** | Jede Änderung landet in einem Wiederherstellungsjournal, bevor sie als erledigt gilt. Ein Absturz kostet höchstens die letzte Sekunde. Board-Dateien werden nie an Ort und Stelle überschrieben. |
| **Speicher folgt einem Budget, nicht der Board-Größe** | Jeder Cache hat ein Limit und verdrängt die am längsten nicht genutzten Bilder. Du wählst den Kompromiss zwischen Zoomgeschwindigkeit und Speicher. |
| **Die Oberfläche wartet nie** | Dekodierung, Festplattenzugriffe und Kompression laufen außerhalb des Hauptthreads. Ein ruhendes Board nutzt 0 % CPU und GPU. |

Boards verwenden die Endung `.moth`. Das Format ist [dokumentiert](docs/FORMAT.md), speichert die Originalbytes deiner Bilder unverändert (ohne Neukodierung) und dedupliziert identische Bilder.

<a id="features"></a>

## Funktionen

Was in der aktuellen Alpha bereits funktioniert:

**Leinwand**
- GPU-Leinwand mit `wgpu` (Vulkan, Metal, DirectX 12) und Software-Rendering als Rückfallebene (`--software`)
- Weicher Zoom zum Mauszeiger, Touch-Verschieben mit Trägheit, Pinch-Zoom und Trackpad-Gesten
- Board-Tabs und ein Startbildschirm mit Board-Bibliothek
- Minimap, editierbare Zoomstufe, Auswahlgröße und Speicherverbrauch in der Statusleiste
- Animationen mit Einstellung für reduzierte Bewegung

**Bilder und Notizen**
- PNG-, JPEG-, WebP-, GIF- und BMP-Dateien einfügen (<kbd>Ctrl</kbd>+<kbd>V</kbd>), hineinziehen oder öffnen
- Textnotizen: Doppelklick auf leere Leinwand erstellt eine, Doppelklick auf eine Notiz bearbeitet sie
- Drehen, Spiegeln, Zuschneiden, Skalieren, Deckkraft und Graustufen, alles rückgängig machbar

**Anordnen**
- Verschieben, mit Alt ziehen zum Duplizieren, mit Pfeiltasten feinjustieren
- Ausrichten, Verteilen, Stapeln, Ebenenreihenfolge und optimales Packen unterschiedlich großer Bilder
- Ganzes Board oder Auswahl mit einem Druck auf <kbd>Space</kbd> einpassen

**Kompatibilität**
- Öffnet **BeeRef**-Boards `.bee`: Bilder, Textnotizen, Drehung, Spiegelung und Skalierung
- Öffnet **PureRef**-Boards `.pur` (1.10 und 2.x): eingebettete Bilder und einfache Notizen; nicht unterstützte Elemente werden gemeldet, nie stillschweigend verworfen

**Export**
- Boards, Auswahlen oder zugeschnittene Bereiche als PNG, SVG und PDF
- Originalbilder genau so, wie sie hinzugefügt wurden, einzeln oder als ZIP
- Farbpipette, die HEX, RGB oder HSV kopiert

**Geschwindigkeit und Speicher**
- Leistungsmodi wie in PureRef: *In memory* (Standard), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Jedes Bild wird pro Zoombereich einmal dekodiert; weitere Zoomstufen nutzen die gecachten Detailstufen
- RAM-Cache auf 25 % des verfügbaren Speichers begrenzt (max. 4 GiB), Festplatten-Cache auf 2 GiB

**Ganz nach deinem Geschmack**
- Alle Tastatur- und Mausbelegungen sind änderbar, mit Vorschau auf Konflikte
- Navigations-Presets im Stil von BeeRef und PureRef
- Befehlspalette (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Oberfläche auf Englisch und Russisch ([eigene Sprache hinzufügen](docs/LOCALIZATION.md))

<details>
<summary><b>Board-Bibliothek (Screenshot)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="MothRef-Board-Bibliothek beim ersten Start" width="900">
</details>

<a id="download"></a>

## Download

Die neueste Version gibt es unter **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| Plattform | Datei | So startest du es |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Entpacken und `mothref.exe` starten |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Entpacken und `mothref` im Terminal starten |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Entpacken und `mothref` im Terminal starten |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Entpacken und `./mothref` starten (Ubuntu 24.04 oder neuer bzw. kompatibel) |

Die Binärdateien sind nicht signiert und die macOS-Builds noch nicht notarisiert, daher fragt dein System beim ersten Start eventuell nach einer Bestätigung. Jedem Release liegen SHA-256-Prüfsummen bei.

Installer und Paketmanager (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR und mehr) sind ab 0.3 [geplant](ROADMAP.md#distribution).
Eine **Android**-App und eine **Web**-Version sind ebenfalls geplant; der Kern ist schon jetzt so geschrieben, dass er sich auf beide portieren lässt.

<a id="getting-around"></a>

## Bedienung

| Aktion | So geht's |
| --- | --- |
| Bilder hinzufügen | Dateien hineinziehen, <kbd>Ctrl</kbd>+<kbd>V</kbd> oder **Paste image** |
| Verschieben | Mit mittlerer Maustaste ziehen, <kbd>Space</kbd>+Ziehen oder mit linker Maustaste und dem Hand-Werkzeug (<kbd>H</kbd>) |
| Zoomen | Mausrad, Pinch oder die Schaltflächen <kbd>+</kbd> / <kbd>−</kbd> |
| Alles einpassen | <kbd>Home</kbd> oder <kbd>Space</kbd> antippen, wenn nichts ausgewählt ist |
| Auswahl einpassen | <kbd>Space</kbd> antippen oder auf ein ausgewähltes Bild doppelklicken (nochmal, um zurückzukehren) |
| Beim Ziehen duplizieren | <kbd>Alt</kbd>+Ziehen |
| Drehen / spiegeln | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Befehlspalette | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Belegungen bearbeiten | <kbd>/</kbd> |

Unter macOS <kbd>Cmd</kbd> statt <kbd>Ctrl</kbd> verwenden. Die vollständige Liste und einen Vergleich mit PureRef findest du in [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (auf Englisch).

<details>
<summary><b>Bekannte Einschränkungen</b></summary>

- Datei-Drag-and-drop unter nativem Wayland ist noch nicht möglich (Einschränkung von winit 0.30). Nutze X11/XWayland, übergib Bilder per Kommandozeile oder füge sie ein.
- Boards unterstützen bis zu 10.000 Elemente; Vorschauen zeigen bis zu 2048 px Detail pro Bild. Zoom in voller Auflösung über Kacheln steht auf der Roadmap.
- Links, formatierte Notizen, Zeichnungen und manche Zuschnitte aus PureRef werden noch nicht importiert.

</details>

<a id="roadmap"></a>

## Roadmap

| Version | Meilenstein | Versprechen | Status |
| --- | --- | --- | --- |
| **0.1** Pre-Alpha | M1 | Verliert keine Daten | Alpha-Builds erschienen |
| **0.3** öffentliche Alpha | M2, M2.5 | Ersetzt BeeRef, öffnet `.bee` und `.pur`, zoomt schneller als PureRef | in Arbeit |
| **0.7** Beta | M3, M4 | Overlay-Modus, Browser-Erweiterung, Funktionsgleichstand mit PureRef 2 | geplant |
| **1.0** | M5–M7 | Board-Bibliothek mit Stapeln und Tags, Elemente im Milanote-Stil, alle Leistungsziele erreicht | geplant |
| **2.0** | M8, M9 | Web-Version und Zusammenarbeit in Echtzeit | später |
| — | — | Android-App | geplant |

Nach 1.0: Werkzeuge für Künstler, Review- und Präsentationsmodus, Obsidian/Markdown-Anbindung und ein Kommandozeilenwerkzeug.
Den vollständigen Plan, Funktionslisten und Leistungsziele findest du in [ROADMAP.md](ROADMAP.md) (auf Englisch).

<a id="build-from-source"></a>

## Aus dem Quellcode bauen

Du brauchst [Rust](https://rustup.rs) 1.95 oder neuer. Unter Linux zusätzlich die X11/Wayland-Header installieren (`libxkbcommon-dev` und `libwayland-dev` unter Debian/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Immer `--release` verwenden: Die Bilddecoder sind auf Optimierungen angewiesen, Debug-Builds sind deutlich langsamer.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # Bilder beim Start öffnen
cargo run -p mothref --release -- board.moth                # ein Board öffnen
cargo run -p mothref --release -- --software                # CPU-Rendering erzwingen
```

<details>
<summary><b>Prüfungen, Benchmarks und Fuzzing</b></summary>

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo xtask corpus
cargo xtask bench --quick
cargo xtask paste
cargo xtask soak 1800
```

`xtask` nutzt optimierte Builds, damit Messungen keinen Debug-Overhead enthalten. Mit `bench --quick --enforce` auf Referenzhardware wird das Frame-Budget von 8 ms (p99) erzwungen, mit `bench --quick --software` ein CPU-Adapter getestet.

Zum Fuzzing der Decoder `cargo-fuzz` und eine Nightly-Toolchain installieren:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

Fuzz-Ziele gibt es auch für `moth_reader`, `bee_reader` und `pur_reader`.

</details>

<details>
<summary><b>Projektstruktur</b></summary>

```
crates/
  mothref/        app: event loop, windows, tabs
  moth-scene/     scene graph, commands, undo/redo        (no GPU, no I/O)
  moth-format/    .moth reader/writer, journal, recovery
  moth-io/        image decoding, detail levels, caches
  moth-render/    wgpu renderer
  moth-ui/        egui interface, theme, motion, localization
  moth-platform/  OS-specific code: clipboard, input, settings
  moth-import/    BeeRef .bee and PureRef .pur import
third_party/purcore/  attributed fork of LazyRef's .pur reader
xtask/          benchmarks, corpus checks, soak tests
corpus/         test boards and images
docs/           format spec, architecture, decisions
```

</details>

<a id="documentation"></a>

## Dokumentation

- [Architektur](docs/ARCHITECTURE.md), [Designentscheidungen](docs/decisions/)
- [Das Dateiformat `.moth`](docs/FORMAT.md)
- [Bedienung und Tastenkürzel-Vergleich mit PureRef](docs/SHORTCUTS.md)
- [Hinweise zum PureRef-Import](docs/PUR-IMPORT.md)
- [Ladeleistung](docs/LOAD-PERFORMANCE.md)
- [Eine Sprache hinzufügen](docs/LOCALIZATION.md)
- [Release-Prozess](docs/RELEASING.md), [Änderungsprotokoll](CHANGELOG.md)

Die Dokumentation ist auf Englisch.

<a id="contributing"></a>

## Mitmachen

Fehlerberichte und Ideen sind in den [Issues](https://github.com/hlophlopgaming/mothRef/issues) willkommen. Lies vor dem Einreichen von Code [AGENTS.md](AGENTS.md): Dort stehen die Architekturregeln, die Regeln zur Datensicherheit und welche Prüfungen auszuführen sind. Sie gelten für Menschen und KI-Agenten gleichermaßen.

<a id="license-and-credits"></a>

## Lizenz und Danksagungen

MothRef ist freie Software unter der **[GNU GPL v3.0 oder neuer](LICENSE)**. Schriftarten und Code von Drittanbietern sind in [THIRD_PARTY.md](THIRD_PARTY.md) aufgeführt.

Der PureRef-Import basiert auf einem Fork von [`purcore`](https://github.com/achrllrogia45/LazyRef) aus LazyRef (MIT). „BeeRef“, „PureRef“ und „Milanote“ werden nur zur Beschreibung der Kompatibilität genannt; MothRef steht in keiner Verbindung zu ihnen.

Die Illustrationen im Screenshot stammen von [David Revoy](https://www.davidrevoy.com): [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) und [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), lizenziert unter [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Die Illustrationen sind für die Anzeige verkleinert.

<div align="center">
<br>
<sub>Gemacht mit Rust und vielen Motten.</sub>
</div>
