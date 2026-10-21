<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="MothRefs logotyp: en nattfjäril i pixelkonst" width="128" height="128">
</picture>

# MothRef

**En snabb och kraschsäker referenstavla för konstnärer.**<br>
Släpp bilder på en oändlig duk, ordna dem och ha dem framför dig medan du arbetar.

[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Ladda ner**](#download) · [Funktioner](#features) · [Kontroller](#getting-around) · [Färdplan](#roadmap) · [Bygga](#build-from-source) · [Dokumentation](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <b>Svenska</b> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="En MothRef-tavla med tre illustrationer på en mörk duk" width="900">

</div>

> [!WARNING]
> MothRef är i **tidig alfa** (`0.1.0-alpha`). Sparande, återställning och ångra fungerar, men långa tillförlitlighetstester återstår. Ha separata kopior av viktiga tavlor och originalbilder.

---

<a id="why-mothref"></a>

## Varför MothRef

[BeeRef](https://github.com/rbreu/beeref) var en fin referenstavla med öppen källkod, men utvecklingen har stannat. [PureRef](https://www.pureref.com) är snabb och genomarbetad, men sluten. MothRef tar vid där BeeRef slutade: nativ, öppen och byggd för enorma tavlor utan att äta upp ditt RAM.

Den vilar på tre löften, i den här ordningen:

| Löfte | Vad det betyder i praktiken |
| --- | --- |
| **Du förlorar aldrig ditt arbete** | Varje ändring skrivs till en återställningsjournal innan den räknas som klar. En krasch kostar som mest den sista sekunden. Tavelfiler skrivs aldrig över på plats. |
| **Minnet följer en budget, inte tavlans storlek** | Varje cache har en gräns och kastar ut de bilder som använts minst nyligen. Du väljer avvägningen mellan zoomhastighet och minne. |
| **Gränssnittet väntar aldrig** | Avkodning, disk och komprimering körs utanför huvudtråden. En tavla i vila använder 0 % CPU och GPU. |

Tavlor sparas med filändelsen `.moth`. Formatet är [dokumenterat](docs/FORMAT.md), lagrar bildernas originalbyte orörda (ingen omkodning) och lagrar identiska bilder bara en gång.

<a id="features"></a>

## Funktioner

Det här fungerar redan i nuvarande alfa:

**Duk**
- GPU-duk med `wgpu` (Vulkan, Metal, DirectX 12) med mjukvarurendering som reserv (`--software`)
- Mjuk zoom mot pekaren, panorering med tröghet på pekskärm, nyp-zoom och styrplattegester
- Flikar för tavlor och en startskärm med tavelbibliotek
- Minikarta, redigerbar zoomnivå, markeringens storlek och minnesanvändning i statusfältet
- Animationer med inställning för minskad rörelse

**Bilder och anteckningar**
- Klistra in (<kbd>Ctrl</kbd>+<kbd>V</kbd>), dra in eller öppna PNG-, JPEG-, WebP-, GIF- och BMP-filer
- Textanteckningar: dubbelklicka på tom duk för att skapa en, dubbelklicka på en anteckning för att redigera den
- Rotera, spegla, beskär, skala, opacitet och gråskala, allt går att ångra

**Ordna**
- Flytta, Alt-dra för att duplicera, finjustera med piltangenterna
- Justera, fördela, stapla, lagerordning och optimal packning av bilder i olika storlekar
- Visa hela tavlan eller markeringen med ett tryck på <kbd>Space</kbd>

**Kompatibilitet**
- Öppnar **BeeRef**-tavlor `.bee`: bilder, textanteckningar, rotation, spegling och skala
- Öppnar **PureRef**-tavlor `.pur` (1.10 och 2.x): inbäddade bilder och enkla anteckningar; element som inte stöds rapporteras och försvinner aldrig i tysthet

**Export**
- Tavlor, markeringar eller beskurna områden till PNG, SVG och PDF
- Originalbilder exakt som de lades till, en och en eller som ZIP
- Färgväljare som kopierar HEX, RGB eller HSV

**Hastighet och minne**
- Prestandalägen som i PureRef: *In memory* (standard), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Varje bild avkodas en gång per zoomintervall; senare zoomsteg återanvänder cachade detaljnivåer
- RAM-cachen är begränsad till 25 % av tillgängligt minne (max 4 GiB), diskcachen till 2 GiB

**Gör den till din**
- Alla tangentbords- och muskommandon går att ändra, med förhandsgranskning av konflikter
- Navigeringsförinställningar i stil med BeeRef och PureRef
- Kommandopalett (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Gränssnitt på engelska och ryska ([lägg till ditt språk](docs/LOCALIZATION.md))

<details>
<summary><b>Tavelbibliotek (skärmbild)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="MothRefs tavelbibliotek vid första start" width="900">
</details>

<a id="download"></a>

## Ladda ner

Hämta senaste versionen från **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| Plattform | Fil | Så kör du den |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Packa upp och kör `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Packa upp och kör `mothref` från Terminal |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Packa upp och kör `mothref` från Terminal |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Packa upp och kör `./mothref` (Ubuntu 24.04 eller nyare, eller kompatibel) |

Binärerna är inte signerade och macOS-versionerna är ännu inte notariserade, så systemet kan be dig bekräfta första starten. SHA-256-kontrollsummor publiceras med varje version.

Installationsprogram och pakethanterare (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR med flera) är [planerade](ROADMAP.md#distribution) från 0.3 och framåt.
En **Android**-app och en **webb**version är också planerade; kärnan skrivs redan så att den kan portas till båda.

<a id="getting-around"></a>

## Kontroller

| För att göra detta | Använd |
| --- | --- |
| Lägga till bilder | Dra in filer, <kbd>Ctrl</kbd>+<kbd>V</kbd> eller **Paste image** |
| Panorera | Dra med mittknappen, <kbd>Space</kbd>+dra, eller dra med vänsterknappen med handverktyget (<kbd>H</kbd>) |
| Zooma | Mushjulet, nyp eller knapparna <kbd>+</kbd> / <kbd>−</kbd> |
| Visa allt | <kbd>Home</kbd>, eller tryck på <kbd>Space</kbd> utan markering |
| Visa markeringen | Tryck på <kbd>Space</kbd> eller dubbelklicka på en markerad bild (igen för att gå tillbaka) |
| Duplicera medan du drar | <kbd>Alt</kbd>+dra |
| Rotera / spegla | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Kommandopalett | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Ändra kortkommandon | <kbd>/</kbd> |

På macOS använder du <kbd>Cmd</kbd> i stället för <kbd>Ctrl</kbd>. Hela listan och en jämförelse med PureRef finns i [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (på engelska).

<details>
<summary><b>Kända begränsningar</b></summary>

- Dra-och-släpp av filer i nativ Wayland fungerar inte än (en begränsning i winit 0.30). Använd X11/XWayland, ange bilder på kommandoraden eller klistra in dem.
- Tavlor stöder upp till 10 000 objekt; förhandsvisningar visar upp till 2048 px detalj per bild. Zoom i full upplösning med rutor finns på färdplanen.
- Länkar, formaterade anteckningar, teckningar och vissa beskärningar från PureRef importeras inte än.

</details>

<a id="roadmap"></a>

## Färdplan

| Version | Milstolpe | Löfte | Status |
| --- | --- | --- | --- |
| **0.1** pre-alfa | M1 | Förlorar inga data | alfaversioner ute |
| **0.3** offentlig alfa | M2, M2.5 | Ersätter BeeRef, öppnar `.bee` och `.pur`, zoomar snabbare än PureRef | pågår |
| **0.7** beta | M3, M4 | Överläggsläge, webbläsartillägg, samma funktioner som PureRef 2 | planerat |
| **1.0** | M5–M7 | Tavelbibliotek med staplar och taggar, element i Milanote-stil, alla prestandamål uppnådda | planerat |
| **2.0** | M8, M9 | Webbversion och samarbete i realtid | senare |
| — | — | Android-app | planerat |

Efter 1.0: verktyg för konstnärer, gransknings- och presentationsläge, Obsidian/Markdown-integration och ett kommandoradsverktyg.
Hela planen, funktionslistor och prestandamål finns i [ROADMAP.md](ROADMAP.md) (på engelska).

<a id="build-from-source"></a>

## Bygga från källkod

Du behöver [Rust](https://rustup.rs) 1.95 eller nyare. På Linux installerar du även X11/Wayland-headers (`libxkbcommon-dev` och `libwayland-dev` på Debian/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Använd alltid `--release`: bildavkodarna är beroende av optimeringar och debug-byggen är mycket långsammare.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # öppna bilder vid start
cargo run -p mothref --release -- board.moth                # öppna en tavla
cargo run -p mothref --release -- --software                # tvinga CPU-rendering
```

<details>
<summary><b>Kontroller, benchmarks och fuzzing</b></summary>

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

`xtask` använder optimerade byggen så att mätningar inte inkluderar debug-overhead. Använd `bench --quick --enforce` på referenshårdvara för att kräva rambudgeten 8 ms (p99), och `bench --quick --software` för att testa en CPU-adapter.

För att fuzza avkodarna installerar du `cargo-fuzz` och en nightly-verktygskedja:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

Det finns även fuzz-mål för `moth_reader`, `bee_reader` och `pur_reader`.

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

- [Arkitektur](docs/ARCHITECTURE.md), [designbeslut](docs/decisions/)
- [Filformatet `.moth`](docs/FORMAT.md)
- [Kontroller och jämförelse av kortkommandon med PureRef](docs/SHORTCUTS.md)
- [Anteckningar om PureRef-import](docs/PUR-IMPORT.md)
- [Laddningsprestanda](docs/LOAD-PERFORMANCE.md)
- [Lägga till ett språk](docs/LOCALIZATION.md)
- [Releaseprocess](docs/RELEASING.md), [ändringslogg](CHANGELOG.md)

Dokumentationen är på engelska.

<a id="contributing"></a>

## Bidra

Felrapporter och idéer är välkomna i [Issues](https://github.com/hlophlopgaming/mothRef/issues). Läs [AGENTS.md](AGENTS.md) innan du skickar kod: där finns arkitekturregler, regler för datasäkerhet och vilka kontroller som ska köras. Den gäller både människor och AI-agenter.

<a id="license-and-credits"></a>

## Licens och tack

MothRef är fri programvara under **[GNU GPL v3.0 eller senare](LICENSE)**. Typsnitt och kod från tredje part finns listade i [THIRD_PARTY.md](THIRD_PARTY.md).

PureRef-importen bygger på en fork av [`purcore`](https://github.com/achrllrogia45/LazyRef) från LazyRef (MIT). ”BeeRef”, ”PureRef” och ”Milanote” nämns bara för att beskriva kompatibilitet; MothRef har ingen koppling till dem.

Illustrationerna i skärmbilden är gjorda av [David Revoy](https://www.davidrevoy.com): [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) och [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), licensierade under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Illustrationerna är nedskalade för visning.

<div align="center">
<br>
<sub>Gjord med Rust och massor av nattfjärilar.</sub>
</div>
