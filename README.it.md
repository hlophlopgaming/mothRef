<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="Logo di MothRef: una falena in pixel art" width="128" height="128">
</picture>

# MothRef

**Una bacheca di riferimenti veloce e a prova di crash per artisti.**<br>
Trascina le immagini su una tela infinita, sistemale e tienile sott'occhio mentre lavori.

[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Scarica**](#download) · [Funzionalità](#features) · [Comandi](#getting-around) · [Roadmap](#roadmap) · [Compilare](#build-from-source) · [Documentazione](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <b>Italiano</b> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="Una bacheca di MothRef con tre illustrazioni su una tela scura" width="900">

</div>

> [!WARNING]
> MothRef è in **alpha iniziale** (`0.1.0-alpha`). Salvataggio, ripristino e annulla funzionano, ma i lunghi test di affidabilità sono ancora da fare. Conserva copie separate delle bacheche importanti e delle immagini originali.

---

<a id="why-mothref"></a>

## Perché MothRef

[BeeRef](https://github.com/rbreu/beeref) era un'ottima bacheca di riferimenti open source, ma il suo sviluppo si è fermato. [PureRef](https://www.pureref.com) è veloce e rifinito, ma chiuso. MothRef riparte da dove si è fermato BeeRef: nativo, aperto e pensato per bacheche enormi senza divorare la RAM.

Si basa su tre promesse, in quest'ordine:

| Promessa | Cosa significa in pratica |
| --- | --- |
| **Non perdi mai il tuo lavoro** | Ogni modifica passa da un giornale di ripristino prima di essere considerata fatta. Un crash costa al massimo l'ultimo secondo. I file delle bacheche non vengono mai sovrascritti sul posto. |
| **La memoria segue un budget, non la dimensione della bacheca** | Ogni cache ha un limite ed elimina le immagini usate meno di recente. Scegli tu il compromesso tra velocità di zoom e memoria. |
| **L'interfaccia non aspetta mai** | Decodifica, disco e compressione girano fuori dal thread principale. Una bacheca inattiva usa lo 0% di CPU e GPU. |

Le bacheche usano l'estensione `.moth`. Il formato è [documentato](docs/FORMAT.md), conserva i byte originali delle immagini senza ricodificarle ed elimina i duplicati identici.

<a id="features"></a>

## Funzionalità

Cosa funziona già nell'alpha attuale:

**Tela**
- Tela su GPU con `wgpu` (Vulkan, Metal, DirectX 12) e rendering software di riserva (`--software`)
- Zoom fluido verso il puntatore, scorrimento touch con inerzia, pizzico e gesti del trackpad
- Schede delle bacheche e una schermata iniziale con la libreria delle bacheche
- Minimappa, livello di zoom modificabile, dimensione della selezione e memoria usata nella barra di stato
- Animazioni con opzione di movimento ridotto

**Immagini e note**
- Incolla (<kbd>Ctrl</kbd>+<kbd>V</kbd>), trascina o apri file PNG, JPEG, WebP, GIF e BMP
- Note di testo: doppio clic sulla tela vuota per crearne una, doppio clic su una nota per modificarla
- Ruota, rifletti, ritaglia, ridimensiona, opacità e scala di grigi, tutto annullabile

**Disposizione**
- Sposta, duplica trascinando con Alt, sposta di precisione con le frecce
- Allinea, distribuisci, impila, ordine dei livelli e impacchettamento ottimale di immagini di dimensioni diverse
- Inquadra tutta la bacheca o la selezione con un tocco di <kbd>Space</kbd>

**Compatibilità**
- Apre le bacheche **BeeRef** `.bee`: immagini, note di testo, rotazione, riflessione e scala
- Apre le bacheche **PureRef** `.pur` (1.10 e 2.x): immagini incorporate e note semplici; gli elementi non supportati vengono segnalati, mai scartati in silenzio

**Esportazione**
- Bacheche, selezioni o aree ritagliate in PNG, SVG e PDF
- Immagini originali esattamente come sono state aggiunte, singolarmente o in ZIP
- Contagocce che copia HEX, RGB o HSV

**Velocità e memoria**
- Modalità di prestazione in stile PureRef: *In memory* (predefinita), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Ogni immagine viene decodificata una volta per intervallo di zoom; i passi successivi riusano i livelli di dettaglio in cache
- Cache in RAM limitata al 25% della memoria disponibile (max 4 GiB), cache su disco a 2 GiB

**Su misura**
- Tutte le scorciatoie di tastiera e mouse sono modificabili, con anteprima dei conflitti
- Preset di navigazione in stile BeeRef e PureRef
- Tavolozza dei comandi (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Interfaccia in inglese e russo ([aggiungi la tua lingua](docs/LOCALIZATION.md))

<details>
<summary><b>Libreria delle bacheche (screenshot)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="Libreria delle bacheche di MothRef al primo avvio" width="900">
</details>

<a id="download"></a>

## Scarica

Scarica l'ultima versione da **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| Piattaforma | File | Come avviarlo |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Estrai ed esegui `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Estrai ed esegui `mothref` dal Terminale |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Estrai ed esegui `mothref` dal Terminale |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Estrai ed esegui `./mothref` (Ubuntu 24.04 o successivo, o compatibile) |

I binari non sono firmati e le build per macOS non sono ancora notarizzate, quindi il sistema potrebbe chiedere conferma al primo avvio. Ogni versione include i checksum SHA-256.

Installer e gestori di pacchetti (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR e altri) sono [previsti](ROADMAP.md#distribution) dalla 0.3 in poi.
Sono previste anche un'app **Android** e una versione **web**; il nucleo è già scritto per essere portabile su entrambe.

<a id="getting-around"></a>

## Comandi

| Per fare questo | Usa |
| --- | --- |
| Aggiungere immagini | Trascina i file, <kbd>Ctrl</kbd>+<kbd>V</kbd> o **Paste image** |
| Spostare la vista | Trascina con il tasto centrale, <kbd>Space</kbd>+trascina, o trascina con il tasto sinistro usando la mano (<kbd>H</kbd>) |
| Zoom | Rotellina, pizzico o i pulsanti <kbd>+</kbd> / <kbd>−</kbd> |
| Inquadrare tutto | <kbd>Home</kbd>, oppure tocca <kbd>Space</kbd> senza selezione |
| Inquadrare la selezione | Tocca <kbd>Space</kbd> o fai doppio clic su un'immagine selezionata (di nuovo per tornare) |
| Duplicare trascinando | <kbd>Alt</kbd>+trascina |
| Ruotare / riflettere | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Tavolozza dei comandi | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Modificare le scorciatoie | <kbd>/</kbd> |

Su macOS usa <kbd>Cmd</kbd> al posto di <kbd>Ctrl</kbd>. L'elenco completo e un confronto con PureRef sono in [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (in inglese).

<details>
<summary><b>Limitazioni note</b></summary>

- Il trascinamento di file su Wayland nativo non è ancora disponibile (limite di winit 0.30). Usa X11/XWayland, passa le immagini da riga di comando o incollale.
- Le bacheche supportano fino a 10.000 elementi; le anteprime mostrano fino a 2048 px di dettaglio per immagine. Lo zoom a piena risoluzione a tasselli è nella roadmap.
- Link, note formattate, disegni e alcuni ritagli di PureRef non vengono ancora importati.

</details>

<a id="roadmap"></a>

## Roadmap

| Versione | Traguardo | Promessa | Stato |
| --- | --- | --- | --- |
| **0.1** pre-alpha | M1 | Non perde dati | alpha pubblicate |
| **0.3** alpha pubblica | M2, M2.5 | Sostituisce BeeRef, apre `.bee` e `.pur`, zooma più velocemente di PureRef | in corso |
| **0.7** beta | M3, M4 | Modalità sovrapposta, estensione per browser, parità di funzioni con PureRef 2 | previsto |
| **1.0** | M5–M7 | Libreria di bacheche con pile ed etichette, elementi in stile Milanote, tutti gli obiettivi di prestazione | previsto |
| **2.0** | M8, M9 | Versione web e collaborazione in tempo reale | più avanti |
| — | — | App Android | previsto |

Dopo la 1.0: strumenti per artisti, modalità revisione e presentazione, integrazione con Obsidian/Markdown e uno strumento da riga di comando.
Il piano completo, le liste di funzioni e gli obiettivi di prestazione sono in [ROADMAP.md](ROADMAP.md) (in inglese).

<a id="build-from-source"></a>

## Compilare dai sorgenti

Serve [Rust](https://rustup.rs) 1.95 o successivo. Su Linux installa anche gli header di X11/Wayland (`libxkbcommon-dev` e `libwayland-dev` su Debian/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Usa sempre `--release`: i decodificatori di immagini dipendono dalle ottimizzazioni e le build di debug sono molto più lente.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # apri immagini all'avvio
cargo run -p mothref --release -- board.moth                # apri una bacheca
cargo run -p mothref --release -- --software                # forza il rendering su CPU
```

<details>
<summary><b>Controlli, benchmark e fuzzing</b></summary>

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

`xtask` usa build ottimizzate così che le misure non includano il sovraccarico del debug. Usa `bench --quick --enforce` sull'hardware di riferimento per imporre il budget di 8 ms per frame (p99), e `bench --quick --software` per provare un adattatore CPU.

Per il fuzzing dei decodificatori, installa `cargo-fuzz` e una toolchain nightly:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

Esistono anche target di fuzzing per `moth_reader`, `bee_reader` e `pur_reader`.

</details>

<details>
<summary><b>Struttura del progetto</b></summary>

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

## Documentazione

- [Architettura](docs/ARCHITECTURE.md), [decisioni di progetto](docs/decisions/)
- [Il formato di file `.moth`](docs/FORMAT.md)
- [Comandi e confronto delle scorciatoie con PureRef](docs/SHORTCUTS.md)
- [Note sull'importazione da PureRef](docs/PUR-IMPORT.md)
- [Prestazioni di caricamento](docs/LOAD-PERFORMANCE.md)
- [Aggiungere una lingua](docs/LOCALIZATION.md)
- [Processo di rilascio](docs/RELEASING.md), [registro delle modifiche](CHANGELOG.md)

La documentazione è in inglese.

<a id="contributing"></a>

## Contribuire

Segnalazioni di bug e idee sono benvenute nelle [Issues](https://github.com/hlophlopgaming/mothRef/issues). Prima di inviare codice, leggi [AGENTS.md](AGENTS.md): spiega le regole di architettura, le regole sulla sicurezza dei dati e quali controlli eseguire. Vale allo stesso modo per persone e agenti IA.

<a id="license-and-credits"></a>

## Licenza e crediti

MothRef è software libero sotto **[GNU GPL v3.0 o successiva](LICENSE)**. Font e codice di terze parti sono elencati in [THIRD_PARTY.md](THIRD_PARTY.md).

L'importazione da PureRef si basa su un fork di [`purcore`](https://github.com/achrllrogia45/LazyRef) di LazyRef (MIT). «BeeRef», «PureRef» e «Milanote» sono citati solo per descrivere la compatibilità; MothRef non è affiliato a essi.

Le illustrazioni nello screenshot sono di [David Revoy](https://www.davidrevoy.com): [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) e [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), con licenza [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Le illustrazioni sono ridimensionate per la visualizzazione.

<div align="center">
<br>
<sub>Fatto con Rust e tante falene.</sub>
</div>
