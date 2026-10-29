<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="Logo MothRef: ćma w stylu pixel art" width="128" height="128">
</picture>

# MothRef

**Szybka i odporna na awarie tablica referencji dla artystów.**<br>
Upuszczaj obrazy na nieskończone płótno, układaj je i miej je na oku podczas pracy.

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Pobierz**](#download) · [Funkcje](#features) · [Sterowanie](#getting-around) · [Plan rozwoju](#roadmap) · [Kompilacja](#build-from-source) · [Dokumentacja](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <b>Polski</b> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="Tablica MothRef z trzema ilustracjami na ciemnym płótnie" width="900">

</div>

> [!WARNING]
> MothRef jest we **wczesnej wersji alfa** (`0.1.0-alpha`). Zapisywanie, odzyskiwanie i cofanie działają, ale długie testy niezawodności są jeszcze przed nami. Trzymaj osobne kopie ważnych tablic i oryginalnych obrazów.

---

<a id="why-mothref"></a>

## Dlaczego MothRef

[BeeRef](https://github.com/rbreu/beeref) był świetną otwartą tablicą referencji, ale jego rozwój się zatrzymał. [PureRef](https://www.pureref.com) jest szybki i dopracowany, ale zamknięty. MothRef kontynuuje to, co zaczął BeeRef: natywny, otwarty i stworzony do ogromnych tablic bez pożerania pamięci RAM.

Opiera się na trzech obietnicach, w tej kolejności:

| Obietnica | Co to oznacza w praktyce |
| --- | --- |
| **Nigdy nie stracisz swojej pracy** | Każda zmiana trafia do dziennika odzyskiwania, zanim zostanie uznana za wykonaną. Awaria kosztuje najwyżej ostatnią sekundę. Pliki tablic nigdy nie są nadpisywane w miejscu. |
| **Pamięć mieści się w budżecie, a nie rośnie z tablicą** | Każda pamięć podręczna ma limit i usuwa najdawniej używane obrazy. Sam wybierasz kompromis między szybkością powiększania a pamięcią. |
| **Interfejs nigdy nie czeka** | Dekodowanie, dysk i kompresja działają poza głównym wątkiem. Bezczynna tablica zużywa 0% CPU i GPU. |

Tablice mają rozszerzenie `.moth`. Format jest [udokumentowany](docs/FORMAT.md), przechowuje oryginalne bajty obrazów bez ponownego kodowania i nie duplikuje identycznych obrazów.

<a id="features"></a>

## Funkcje

Co już działa w obecnej wersji alfa:

**Płótno**
- Płótno na GPU z `wgpu` (Vulkan, Metal, DirectX 12) z zapasowym renderowaniem programowym (`--software`)
- Płynne powiększanie w stronę kursora, przewijanie dotykiem z bezwładnością, szczypanie i gesty touchpada
- Karty tablic i ekran startowy z biblioteką tablic
- Minimapa, edytowalny poziom powiększenia, rozmiar zaznaczenia i zużycie pamięci na pasku stanu
- Animacje z opcją ograniczenia ruchu

**Obrazy i notatki**
- Wklejaj (<kbd>Ctrl</kbd>+<kbd>V</kbd>), przeciągaj lub otwieraj pliki PNG, JPEG, WebP, GIF i BMP
- Notatki tekstowe: dwuklik na pustym płótnie tworzy notatkę, dwuklik na notatce otwiera edycję
- Obracanie, odbijanie, kadrowanie, skalowanie, przezroczystość i skala szarości, wszystko z cofaniem

**Układanie**
- Przesuwanie, duplikowanie przeciąganiem z Alt, przesuwanie strzałkami
- Wyrównywanie, rozmieszczanie, układanie w stos, kolejność warstw i optymalne upakowanie obrazów o różnych rozmiarach
- Dopasuj całą tablicę lub zaznaczenie jednym naciśnięciem <kbd>Space</kbd>

**Zgodność**
- Otwiera tablice **BeeRef** `.bee`: obrazy, notatki tekstowe, obrót, odbicie i skalę
- Otwiera tablice **PureRef** `.pur` (1.10 i 2.x): osadzone obrazy i proste notatki; nieobsługiwane elementy są zgłaszane, nigdy nie znikają po cichu

**Eksport**
- Tablice, zaznaczenia lub wykadrowane obszary do PNG, SVG i PDF
- Oryginalne obrazy dokładnie takie, jak zostały dodane, pojedynczo lub jako ZIP
- Próbnik kolorów kopiujący HEX, RGB lub HSV

**Szybkość i pamięć**
- Tryby wydajności jak w PureRef: *In memory* (domyślny), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Każdy obraz jest dekodowany raz na zakres powiększenia; kolejne kroki korzystają z zapisanych poziomów szczegółowości
- Pamięć podręczna w RAM ograniczona do 25% dostępnej pamięci (maks. 4 GiB), na dysku do 2 GiB

**Po swojemu**
- Każdy skrót klawiatury i myszy można zmienić, z podglądem konfliktów
- Presety nawigacji w stylu BeeRef i PureRef
- Paleta poleceń (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Interfejs po angielsku i rosyjsku ([dodaj swój język](docs/LOCALIZATION.md))

<details>
<summary><b>Biblioteka tablic (zrzut ekranu)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="Biblioteka tablic MothRef przy pierwszym uruchomieniu" width="900">
</details>

<a id="download"></a>

## Pobierz

Najnowszą wersję pobierzesz z **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| Platforma | Plik | Jak uruchomić |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Rozpakuj i uruchom `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Rozpakuj i uruchom `mothref` z Terminala |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Rozpakuj i uruchom `mothref` z Terminala |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Rozpakuj i uruchom `./mothref` (Ubuntu 24.04 lub nowszy albo zgodny) |

Pliki nie są podpisane, a wersje dla macOS nie są jeszcze notaryzowane, więc system może poprosić o potwierdzenie przy pierwszym uruchomieniu. Do każdego wydania dołączone są sumy kontrolne SHA-256.

Instalatory i menedżery pakietów (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR i inne) są [planowane](ROADMAP.md#distribution) od wersji 0.3.
Planowane są też aplikacja na **Androida** i wersja **webowa**; rdzeń już jest pisany tak, by dało się go przenieść na obie platformy.

<a id="getting-around"></a>

## Sterowanie

| Aby to zrobić | Użyj |
| --- | --- |
| Dodawanie obrazów | Przeciągnij pliki, <kbd>Ctrl</kbd>+<kbd>V</kbd> lub **Paste image** |
| Przesuwanie widoku | Przeciąganie środkowym przyciskiem, <kbd>Space</kbd>+przeciąganie lub lewym przyciskiem z narzędziem Ręka (<kbd>H</kbd>) |
| Powiększanie | Kółko myszy, szczypanie lub przyciski <kbd>+</kbd> / <kbd>−</kbd> |
| Pokaż wszystko | <kbd>Home</kbd> lub naciśnij <kbd>Space</kbd> bez zaznaczenia |
| Pokaż zaznaczenie | Naciśnij <kbd>Space</kbd> lub kliknij dwukrotnie zaznaczony obraz (ponownie, aby wrócić) |
| Duplikowanie przy przeciąganiu | <kbd>Alt</kbd>+przeciąganie |
| Obróć / odbij | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Paleta poleceń | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Edycja skrótów | <kbd>/</kbd> |

Na macOS używaj <kbd>Cmd</kbd> zamiast <kbd>Ctrl</kbd>. Pełna lista i porównanie z PureRef są w [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (po angielsku).

<details>
<summary><b>Znane ograniczenia</b></summary>

- Przeciąganie plików w natywnym Waylandzie nie jest jeszcze dostępne (ograniczenie winit 0.30). Użyj X11/XWayland, przekaż obrazy w wierszu poleceń lub je wklej.
- Tablice obsługują do 10 000 elementów; podglądy pokazują do 2048 px szczegółów na obraz. Powiększanie w pełnej rozdzielczości kafelkami jest w planach.
- Linki, sformatowane notatki, rysunki i część kadrowań z PureRef nie są jeszcze importowane.

</details>

<a id="roadmap"></a>

## Plan rozwoju

| Wydanie | Etap | Obietnica | Status |
| --- | --- | --- | --- |
| **0.1** pre-alfa | M1 | Nie traci danych | wydania alfa dostępne |
| **0.3** publiczna alfa | M2, M2.5 | Zastępuje BeeRef, otwiera `.bee` i `.pur`, powiększa szybciej niż PureRef | w toku |
| **0.7** beta | M3, M4 | Tryb nakładki, wszystkie funkcje PureRef 2 | planowane |
| **1.0** | M5–M7 | Biblioteka tablic ze stosami i tagami, elementy w stylu Milanote, wszystkie cele wydajności | planowane |
| **2.0** | M8, M9 | Wersja webowa i współpraca w czasie rzeczywistym | później |
| — | — | Aplikacja na Androida | planowane |

Po 1.0: narzędzia dla artystów, tryb recenzji i prezentacji, integracja z Obsidianem/Markdownem i narzędzie wiersza poleceń.
Pełny plan, listy funkcji i cele wydajności są w [ROADMAP.md](ROADMAP.md) (po angielsku).

<a id="build-from-source"></a>

## Kompilacja ze źródeł

Potrzebujesz [Rusta](https://rustup.rs) w wersji 1.95 lub nowszej. Na Linuksie zainstaluj też nagłówki X11/Wayland (`libxkbcommon-dev`, `libdbus-1-dev` i `libwayland-dev` w Debianie/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Zawsze używaj `--release`: dekodery obrazów polegają na optymalizacjach, a kompilacje debug są znacznie wolniejsze.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # otwórz obrazy przy starcie
cargo run -p mothref --release -- board.moth                # otwórz tablicę
cargo run -p mothref --release -- --software                # wymuś renderowanie na CPU
```

<details>
<summary><b>Testy, benchmarki i fuzzing</b></summary>

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

`xtask` używa zoptymalizowanych kompilacji, żeby pomiary nie obejmowały narzutu trybu debug. `bench --quick --enforce` na sprzęcie referencyjnym wymusza budżet 8 ms na klatkę (p99), a `bench --quick --software` sprawdza adapter CPU.

Do fuzzingu dekoderów zainstaluj `cargo-fuzz` i toolchain nightly:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

Istnieją też cele fuzzingu `moth_reader`, `bee_reader` i `pur_reader`.

</details>

<details>
<summary><b>Struktura projektu</b></summary>

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

## Dokumentacja

- [Architektura](docs/ARCHITECTURE.md), [decyzje projektowe](docs/decisions/)
- [Format plików `.moth`](docs/FORMAT.md)
- [Sterowanie i porównanie skrótów z PureRef](docs/SHORTCUTS.md)
- [Uwagi o imporcie z PureRef](docs/PUR-IMPORT.md)
- [Wydajność wczytywania](docs/LOAD-PERFORMANCE.md)
- [Dodawanie języka](docs/LOCALIZATION.md)
- [Proces wydawania](docs/RELEASING.md), [lista zmian](CHANGELOG.md)

Dokumentacja jest po angielsku.

<a id="contributing"></a>

## Współpraca

Zgłoszenia błędów i pomysły są mile widziane w [Issues](https://github.com/hlophlopgaming/mothRef/issues). Zanim wyślesz kod, przeczytaj [AGENTS.md](AGENTS.md): opisuje zasady architektury, zasady bezpieczeństwa danych i wymagane testy. Dotyczy zarówno ludzi, jak i agentów AI.

<a id="license-and-credits"></a>

## Licencja i podziękowania

MothRef to wolne oprogramowanie na licencji **[GNU GPL v3.0 lub nowszej](LICENSE)**. Czcionki i kod firm trzecich wymieniono w [THIRD_PARTY.md](THIRD_PARTY.md).

Import z PureRef opiera się na forku [`purcore`](https://github.com/achrllrogia45/LazyRef) z LazyRef (MIT). Nazwy „BeeRef”, „PureRef” i „Milanote” podano wyłącznie w celu opisania zgodności; MothRef nie jest z nimi powiązany.

Ilustracje na zrzucie ekranu są autorstwa [Davida Revoya](https://www.davidrevoy.com): [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) i [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), na licencji [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Ilustracje zostały pomniejszone do wyświetlenia.

<div align="center">
<br>
<sub>Zrobione w Ruście i z mnóstwem ciem.</sub>
</div>
