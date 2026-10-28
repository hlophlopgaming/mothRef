<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="Logotipo de MothRef: una polilla en pixel art" width="128" height="128">
</picture>

# MothRef

**Un tablero de referencias rápido y a prueba de fallos para artistas.**<br>
Suelta imágenes en un lienzo infinito, ordénalas y tenlas a la vista mientras trabajas.

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Descargar**](#download) · [Funciones](#features) · [Controles](#getting-around) · [Hoja de ruta](#roadmap) · [Compilar](#build-from-source) · [Documentación](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <b>Español</b> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="Un tablero de MothRef con tres ilustraciones sobre un lienzo oscuro" width="900">

</div>

> [!WARNING]
> MothRef está en **alfa temprana** (`0.1.0-alpha`). Guardar, recuperar y deshacer funcionan, pero aún faltan las pruebas largas de fiabilidad. Guarda copias aparte de tus tableros importantes y de las imágenes originales.

---

<a id="why-mothref"></a>

## Por qué MothRef

[BeeRef](https://github.com/rbreu/beeref) era un tablero de referencias libre estupendo, pero dejó de avanzar. [PureRef](https://www.pureref.com) es rápido y pulido, pero cerrado. MothRef retoma lo que dejó BeeRef: nativo, abierto y pensado para tableros enormes sin devorar la RAM.

Se basa en tres promesas, en este orden:

| Promesa | Qué significa en la práctica |
| --- | --- |
| **Nunca pierdes tu trabajo** | Cada cambio pasa por un diario de recuperación antes de darse por hecho. Un cierre inesperado cuesta, como mucho, el último segundo. Los archivos de tablero nunca se sobrescriben en el sitio. |
| **La memoria sigue un presupuesto, no el tamaño del tablero** | Cada caché tiene un límite y descarta las imágenes usadas hace más tiempo. Tú eliges el equilibrio entre velocidad de zoom y memoria. |
| **La interfaz nunca espera** | La decodificación, el disco y la compresión van fuera del hilo principal. Un tablero en reposo usa un 0% de CPU y GPU. |

Los tableros se guardan como archivos `.moth`. El formato está [documentado](docs/FORMAT.md), conserva los bytes originales de tus imágenes sin recodificarlas y no duplica imágenes idénticas.

<a id="features"></a>

## Funciones

Lo que ya funciona en la alfa actual:

**Lienzo**
- Lienzo por GPU con `wgpu` (Vulkan, Metal, DirectX 12) y renderizado por software de respaldo (`--software`)
- Zoom suave hacia el puntero, desplazamiento táctil con inercia, pellizco y gestos de trackpad
- Pestañas de tableros y una pantalla de inicio con la biblioteca de tableros
- Minimapa, nivel de zoom editable, tamaño de la selección y uso de memoria en la barra de estado
- Animaciones con opción de reducir el movimiento

**Imágenes y notas**
- Pega (<kbd>Ctrl</kbd>+<kbd>V</kbd>), arrastra o abre archivos PNG, JPEG, WebP, GIF y BMP
- Notas de texto: doble clic en el lienzo vacío para crear una, doble clic en una nota para editarla
- Girar, voltear, recortar, escalar, opacidad y escala de grises, todo con deshacer

**Organizar**
- Mover, duplicar arrastrando con Alt, desplazar con las flechas
- Alinear, distribuir, apilar, orden de capas y empaquetado óptimo de imágenes de distintos tamaños
- Encuadrar todo el tablero o la selección con un toque de <kbd>Space</kbd>

**Compatibilidad**
- Abre tableros **BeeRef** `.bee`: imágenes, notas de texto, rotación, volteo y escala
- Abre tableros **PureRef** `.pur` (1.10 y 2.x): imágenes incrustadas y notas simples; los elementos no compatibles se notifican, nunca se pierden en silencio

**Exportar**
- Tableros, selecciones o zonas recortadas a PNG, SVG y PDF
- Las imágenes originales tal como se añadieron, una a una o en un ZIP
- Cuentagotas que copia HEX, RGB o HSV

**Velocidad y memoria**
- Modos de rendimiento al estilo de PureRef: *In memory* (por defecto), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Cada imagen se decodifica una vez por rango de zoom; los siguientes pasos reutilizan los niveles de detalle en caché
- Caché en RAM limitada al 25% de la memoria disponible (máx. 4 GiB), caché en disco a 2 GiB

**A tu manera**
- Todos los atajos de teclado y ratón se pueden editar, con vista previa de conflictos
- Preajustes de navegación al estilo de BeeRef y de PureRef
- Paleta de comandos (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Interfaz en inglés y ruso ([añade tu idioma](docs/LOCALIZATION.md))

<details>
<summary><b>Biblioteca de tableros (captura)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="Biblioteca de tableros de MothRef en el primer arranque" width="900">
</details>

<a id="download"></a>

## Descargar

Descarga la última versión desde **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| Plataforma | Archivo | Cómo ejecutarlo |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Descomprime y ejecuta `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Descomprime y ejecuta `mothref` desde Terminal |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Descomprime y ejecuta `mothref` desde Terminal |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Descomprime y ejecuta `./mothref` (Ubuntu 24.04 o posterior, o compatible) |

Los binarios no están firmados y las versiones de macOS aún no están notarizadas, así que el sistema puede pedirte confirmación al primer arranque. Cada versión incluye sumas de verificación SHA-256.

Los instaladores y gestores de paquetes (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR y más) están [planificados](ROADMAP.md#distribution) a partir de la 0.3.
También están planificadas una app para **Android** y una versión **web**; el núcleo ya se escribe para que sea portable a ambas.

<a id="getting-around"></a>

## Controles

| Para hacer esto | Usa |
| --- | --- |
| Añadir imágenes | Arrastra archivos, <kbd>Ctrl</kbd>+<kbd>V</kbd> o **Paste image** |
| Desplazar | Arrastrar con el botón central, <kbd>Space</kbd>+arrastrar o arrastrar con el botón izquierdo usando la mano (<kbd>H</kbd>) |
| Zoom | Rueda del ratón, pellizco o los botones <kbd>+</kbd> / <kbd>−</kbd> |
| Encuadrar todo | <kbd>Home</kbd>, o toca <kbd>Space</kbd> sin nada seleccionado |
| Encuadrar la selección | Toca <kbd>Space</kbd> o haz doble clic en una imagen seleccionada (otra vez para volver) |
| Duplicar al arrastrar | <kbd>Alt</kbd>+arrastrar |
| Girar / voltear | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Paleta de comandos | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Editar atajos | <kbd>/</kbd> |

En macOS, usa <kbd>Cmd</kbd> en lugar de <kbd>Ctrl</kbd>. La lista completa y una comparación con PureRef están en [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (en inglés).

<details>
<summary><b>Limitaciones conocidas</b></summary>

- Arrastrar archivos en Wayland nativo aún no es posible (limitación de winit 0.30). Usa X11/XWayland, pasa las imágenes por la línea de comandos o pégalas.
- Los tableros admiten hasta 10 000 elementos; las vistas previas muestran hasta 2048 px de detalle por imagen. El zoom a resolución completa por mosaicos está en la hoja de ruta.
- Los enlaces, las notas con formato, los dibujos y algunos recortes de PureRef aún no se importan.

</details>

<a id="roadmap"></a>

## Hoja de ruta

| Versión | Hito | Promesa | Estado |
| --- | --- | --- | --- |
| **0.1** prealfa | M1 | No pierde datos | alfas publicadas |
| **0.3** alfa pública | M2, M2.5 | Sustituye a BeeRef, abre `.bee` y `.pur`, hace zoom más rápido que PureRef | en curso |
| **0.7** beta | M3, M4 | Modo superpuesto, extensión de navegador, paridad de funciones con PureRef 2 | planificado |
| **1.0** | M5–M7 | Biblioteca de tableros con pilas y etiquetas, elementos al estilo de Milanote, todos los objetivos de rendimiento | planificado |
| **2.0** | M8, M9 | Versión web y colaboración en tiempo real | más adelante |
| — | — | App para Android | planificado |

Después de la 1.0: herramientas para artistas, modo de revisión y presentación, integración con Obsidian/Markdown y una herramienta de línea de comandos.
El plan completo, las listas de funciones y los objetivos de rendimiento están en [ROADMAP.md](ROADMAP.md) (en inglés).

<a id="build-from-source"></a>

## Compilar desde el código

Necesitas [Rust](https://rustup.rs) 1.95 o posterior. En Linux, instala también las cabeceras de X11/Wayland (`libxkbcommon-dev` y `libwayland-dev` en Debian/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Usa siempre `--release`: los decodificadores de imágenes dependen de las optimizaciones y las compilaciones de depuración son mucho más lentas.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # abrir imágenes al iniciar
cargo run -p mothref --release -- board.moth                # abrir un tablero
cargo run -p mothref --release -- --software                # forzar el renderizado por CPU
```

<details>
<summary><b>Comprobaciones, benchmarks y fuzzing</b></summary>

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

`xtask` usa compilaciones optimizadas para que las mediciones no incluyan la sobrecarga de depuración. Usa `bench --quick --enforce` en el hardware de referencia para exigir el presupuesto de 8 ms por fotograma (p99), y `bench --quick --software` para probar un adaptador por CPU.

Para hacer fuzzing de los decodificadores, instala `cargo-fuzz` y una toolchain nightly:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

También hay objetivos de fuzzing para `moth_reader`, `bee_reader` y `pur_reader`.

</details>

<details>
<summary><b>Estructura del proyecto</b></summary>

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

## Documentación

- [Arquitectura](docs/ARCHITECTURE.md), [decisiones de diseño](docs/decisions/)
- [El formato de archivo `.moth`](docs/FORMAT.md)
- [Controles y comparación de atajos con PureRef](docs/SHORTCUTS.md)
- [Notas sobre la importación de PureRef](docs/PUR-IMPORT.md)
- [Rendimiento de carga](docs/LOAD-PERFORMANCE.md)
- [Añadir un idioma](docs/LOCALIZATION.md)
- [Proceso de publicación](docs/RELEASING.md), [registro de cambios](CHANGELOG.md)

La documentación está en inglés.

<a id="contributing"></a>

## Contribuir

Los informes de errores y las ideas son bienvenidos en [Issues](https://github.com/hlophlopgaming/mothRef/issues). Antes de enviar código, lee [AGENTS.md](AGENTS.md): explica las reglas de arquitectura, las reglas de seguridad de los datos y qué comprobaciones ejecutar. Se aplica por igual a personas y a agentes de IA.

<a id="license-and-credits"></a>

## Licencia y créditos

MothRef es software libre bajo la **[GNU GPL v3.0 o posterior](LICENSE)**. Las fuentes y el código de terceros figuran en [THIRD_PARTY.md](THIRD_PARTY.md).

La importación de PureRef se basa en un fork de [`purcore`](https://github.com/achrllrogia45/LazyRef) de LazyRef (MIT). «BeeRef», «PureRef» y «Milanote» se mencionan solo para describir la compatibilidad; MothRef no está afiliado a ellos.

Las ilustraciones de la captura son de [David Revoy](https://www.davidrevoy.com): [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) y [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), con licencia [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Las ilustraciones se han reducido para mostrarlas.

<div align="center">
<br>
<sub>Hecho con Rust y muchas polillas.</sub>
</div>
