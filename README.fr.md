<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="Logo de MothRef : un papillon de nuit en pixel art" width="128" height="128">
</picture>

# MothRef

**Un tableau de références rapide et à l'épreuve des plantages, pour les artistes.**<br>
Déposez des images sur une toile infinie, organisez-les et gardez-les sous les yeux pendant que vous travaillez.

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Télécharger**](#download) · [Fonctionnalités](#features) · [Commandes](#getting-around) · [Feuille de route](#roadmap) · [Compiler](#build-from-source) · [Documentation](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <b>Français</b> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="Un tableau MothRef avec trois illustrations sur une toile sombre" width="900">

</div>

> [!WARNING]
> MothRef est en **alpha précoce** (`0.1.0-alpha`). L'enregistrement, la récupération et l'annulation fonctionnent, mais les longs tests de fiabilité restent à faire. Gardez des copies séparées de vos tableaux importants et des images d'origine.

---

<a id="why-mothref"></a>

## Pourquoi MothRef

[BeeRef](https://github.com/rbreu/beeref) était un excellent tableau de références libre, mais son développement s'est arrêté. [PureRef](https://www.pureref.com) est rapide et soigné, mais fermé. MothRef reprend là où BeeRef s'est arrêté : natif, ouvert et conçu pour d'immenses tableaux sans dévorer votre RAM.

Il repose sur trois promesses, dans cet ordre :

| Promesse | Ce que cela veut dire en pratique |
| --- | --- |
| **Vous ne perdez jamais votre travail** | Chaque modification passe par un journal de récupération avant d'être considérée comme faite. Un plantage coûte au plus la dernière seconde. Les fichiers de tableau ne sont jamais écrasés sur place. |
| **La mémoire suit un budget, pas la taille du tableau** | Chaque cache a une limite et évince les images utilisées le moins récemment. Vous choisissez le compromis entre vitesse de zoom et mémoire. |
| **L'interface n'attend jamais** | Le décodage, le disque et la compression tournent hors du thread principal. Un tableau inactif utilise 0 % du CPU et du GPU. |

Les tableaux utilisent l'extension `.moth`. Le format est [documenté](docs/FORMAT.md), conserve les octets d'origine de vos images sans les réencoder et ne duplique pas les images identiques.

<a id="features"></a>

## Fonctionnalités

Ce qui fonctionne déjà dans l'alpha actuelle :

**Toile**
- Toile sur GPU avec `wgpu` (Vulkan, Metal, DirectX 12) et rendu logiciel de secours (`--software`)
- Zoom fluide vers le pointeur, défilement tactile avec inertie, pincement et gestes du pavé tactile
- Onglets de tableaux et un écran d'accueil avec la bibliothèque de tableaux
- Mini-carte, niveau de zoom modifiable, taille de la sélection et mémoire utilisée dans la barre d'état
- Animations avec option de mouvement réduit

**Images et notes**
- Collez (<kbd>Ctrl</kbd>+<kbd>V</kbd>), glissez-déposez ou ouvrez des fichiers PNG, JPEG, WebP, GIF et BMP
- Notes texte : double-clic sur la toile vide pour en créer une, double-clic sur une note pour la modifier
- Rotation, miroir, recadrage, mise à l'échelle, opacité et niveaux de gris, tout est annulable

**Organisation**
- Déplacer, dupliquer en glissant avec Alt, ajuster avec les flèches
- Aligner, répartir, empiler, ordre des calques et rangement optimal d'images de tailles différentes
- Cadrer tout le tableau ou la sélection d'une pression sur <kbd>Space</kbd>

**Compatibilité**
- Ouvre les tableaux **BeeRef** `.bee` : images, notes texte, rotation, miroir et échelle
- Ouvre les tableaux **PureRef** `.pur` (1.10 et 2.x) : images intégrées et notes simples ; les éléments non pris en charge sont signalés, jamais supprimés en silence

**Export**
- Tableaux, sélections ou zones recadrées en PNG, SVG et PDF
- Images d'origine telles qu'elles ont été ajoutées, une par une ou en ZIP
- Pipette qui copie en HEX, RVB ou TSV

**Vitesse et mémoire**
- Modes de performance à la PureRef : *In memory* (par défaut), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Chaque image est décodée une fois par plage de zoom ; les paliers suivants réutilisent les niveaux de détail en cache
- Cache en RAM plafonné à 25 % de la mémoire disponible (4 Gio max.), cache disque à 2 Gio

**À votre façon**
- Tous les raccourcis clavier et souris sont modifiables, avec aperçu des conflits
- Préréglages de navigation façon BeeRef et façon PureRef
- Palette de commandes (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Interface en anglais et en russe ([ajoutez votre langue](docs/LOCALIZATION.md))

<details>
<summary><b>Bibliothèque de tableaux (capture d'écran)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="Bibliothèque de tableaux de MothRef au premier lancement" width="900">
</details>

<a id="download"></a>

## Télécharger

Récupérez la dernière version sur **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| Plateforme | Fichier | Comment lancer |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Décompressez et lancez `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Décompressez et lancez `mothref` depuis le Terminal |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Décompressez et lancez `mothref` depuis le Terminal |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Décompressez et lancez `./mothref` (Ubuntu 24.04 ou plus récent, ou compatible) |

Les binaires ne sont pas signés et les versions macOS ne sont pas encore notariées : votre système peut demander une confirmation au premier lancement. Des sommes de contrôle SHA-256 accompagnent chaque version.

Les installeurs et gestionnaires de paquets (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR, etc.) sont [prévus](ROADMAP.md#distribution) à partir de la 0.3.
Une application **Android** et une version **web** sont aussi prévues ; le cœur est déjà écrit pour rester portable vers les deux.

<a id="getting-around"></a>

## Commandes

| Pour faire ceci | Utilisez |
| --- | --- |
| Ajouter des images | Glissez des fichiers, <kbd>Ctrl</kbd>+<kbd>V</kbd> ou **Paste image** |
| Se déplacer | Glisser avec le bouton du milieu, <kbd>Space</kbd>+glisser, ou glisser avec le bouton gauche avec l'outil Main (<kbd>H</kbd>) |
| Zoomer | Molette, pincement ou les boutons <kbd>+</kbd> / <kbd>−</kbd> |
| Tout cadrer | <kbd>Home</kbd>, ou appuyez sur <kbd>Space</kbd> sans sélection |
| Cadrer la sélection | Appuyez sur <kbd>Space</kbd> ou double-cliquez sur une image sélectionnée (encore une fois pour revenir) |
| Dupliquer en glissant | <kbd>Alt</kbd>+glisser |
| Pivoter / retourner | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Palette de commandes | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Modifier les raccourcis | <kbd>/</kbd> |

Sur macOS, utilisez <kbd>Cmd</kbd> au lieu de <kbd>Ctrl</kbd>. La liste complète et une comparaison avec PureRef se trouvent dans [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (en anglais).

<details>
<summary><b>Limites connues</b></summary>

- Le glisser-déposer de fichiers sous Wayland natif n'est pas encore possible (limite de winit 0.30). Utilisez X11/XWayland, passez les images en ligne de commande ou collez-les.
- Les tableaux acceptent jusqu'à 10 000 éléments ; les aperçus affichent jusqu'à 2048 px de détail par image. Le zoom en pleine résolution par tuiles est sur la feuille de route.
- Les liens, notes enrichies, dessins et certains recadrages de PureRef ne sont pas encore importés.

</details>

<a id="roadmap"></a>

## Feuille de route

| Version | Étape | Promesse | État |
| --- | --- | --- | --- |
| **0.1** pré-alpha | M1 | Ne perd pas de données | versions alpha publiées |
| **0.3** alpha publique | M2, M2.5 | Remplace BeeRef, ouvre `.bee` et `.pur`, zoome plus vite que PureRef | en cours |
| **0.7** bêta | M3, M4 | Mode superposition, extension de navigateur, parité de fonctionnalités avec PureRef 2 | prévu |
| **1.0** | M5–M7 | Bibliothèque de tableaux avec piles et étiquettes, éléments façon Milanote, tous les objectifs de performance atteints | prévu |
| **2.0** | M8, M9 | Version web et collaboration en temps réel | plus tard |
| — | — | Application Android | prévu |

Après la 1.0 : outils pour artistes, mode revue et présentation, intégration Obsidian/Markdown et un outil en ligne de commande.
Le plan complet, les listes de fonctionnalités et les objectifs de performance sont dans [ROADMAP.md](ROADMAP.md) (en anglais).

<a id="build-from-source"></a>

## Compiler depuis les sources

Il vous faut [Rust](https://rustup.rs) 1.95 ou plus récent. Sous Linux, installez aussi les en-têtes X11/Wayland (`libxkbcommon-dev` et `libwayland-dev` sous Debian/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Utilisez toujours `--release` : les décodeurs d'images dépendent des optimisations et les builds de débogage sont bien plus lentes.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # ouvrir des images au démarrage
cargo run -p mothref --release -- board.moth                # ouvrir un tableau
cargo run -p mothref --release -- --software                # forcer le rendu par le CPU
```

<details>
<summary><b>Vérifications, benchmarks et fuzzing</b></summary>

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

`xtask` utilise des builds optimisées pour que les mesures n'incluent pas le surcoût du débogage. Utilisez `bench --quick --enforce` sur le matériel de référence pour imposer le budget de 8 ms par image (p99), et `bench --quick --software` pour tester un adaptateur CPU.

Pour fuzzer les décodeurs, installez `cargo-fuzz` et une toolchain nightly :

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

Des cibles de fuzzing existent aussi pour `moth_reader`, `bee_reader` et `pur_reader`.

</details>

<details>
<summary><b>Organisation du projet</b></summary>

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

## Documentation

- [Architecture](docs/ARCHITECTURE.md), [décisions de conception](docs/decisions/)
- [Le format de fichier `.moth`](docs/FORMAT.md)
- [Commandes et comparaison des raccourcis avec PureRef](docs/SHORTCUTS.md)
- [Notes sur l'import PureRef](docs/PUR-IMPORT.md)
- [Performances de chargement](docs/LOAD-PERFORMANCE.md)
- [Ajouter une langue](docs/LOCALIZATION.md)
- [Processus de publication](docs/RELEASING.md), [journal des modifications](CHANGELOG.md)

La documentation est en anglais.

<a id="contributing"></a>

## Contribuer

Les rapports de bugs et les idées sont les bienvenus dans les [Issues](https://github.com/hlophlopgaming/mothRef/issues). Avant d'envoyer du code, lisez [AGENTS.md](AGENTS.md) : il décrit les règles d'architecture, les règles de sécurité des données et les vérifications à lancer. Il s'applique autant aux humains qu'aux agents d'IA.

<a id="license-and-credits"></a>

## Licence et crédits

MothRef est un logiciel libre sous **[GNU GPL v3.0 ou ultérieure](LICENSE)**. Les polices et le code tiers sont listés dans [THIRD_PARTY.md](THIRD_PARTY.md).

L'import PureRef s'appuie sur un fork de [`purcore`](https://github.com/achrllrogia45/LazyRef) issu de LazyRef (MIT). « BeeRef », « PureRef » et « Milanote » ne sont cités que pour décrire la compatibilité ; MothRef n'y est pas affilié.

Les illustrations de la capture sont de [David Revoy](https://www.davidrevoy.com) : [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) et [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), sous licence [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Les illustrations sont réduites pour l'affichage.

<div align="center">
<br>
<sub>Fait avec Rust et beaucoup de papillons de nuit.</sub>
</div>
