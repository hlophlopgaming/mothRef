<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="MothRefin logo: pikselitaiteella piirretty koiperhonen" width="128" height="128">
</picture>

# MothRef

**Nopea ja kaatumisilta suojattu referenssitaulu taiteilijoille.**<br>
Pudota kuvat äärettömälle kankaalle, järjestä ne ja pidä ne näkyvillä työskennellessäsi.

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Lataa**](#download) · [Ominaisuudet](#features) · [Ohjaus](#getting-around) · [Tiekartta](#roadmap) · [Kääntäminen](#build-from-source) · [Dokumentaatio](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <b>Suomi</b> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="MothRef-taulu, jossa on kolme kuvitusta tummalla kankaalla" width="900">

</div>

> [!WARNING]
> MothRef on **varhaisessa alfavaiheessa** (`0.1.0-alpha`). Tallennus, palautus ja kumoaminen toimivat, mutta pitkät luotettavuustestit ovat vielä edessä. Pidä tärkeistä tauluista ja alkuperäisistä kuvista erilliset kopiot.

---

<a id="why-mothref"></a>

## Miksi MothRef

[BeeRef](https://github.com/rbreu/beeref) oli hieno avoimen lähdekoodin referenssitaulu, mutta sen kehitys pysähtyi. [PureRef](https://www.pureref.com) on nopea ja viimeistelty, mutta suljettu. MothRef jatkaa siitä, mihin BeeRef jäi: natiivi, avoin ja tehty valtaville tauluille syömättä koneen muistia.

Se rakentuu kolmen lupauksen varaan, tässä järjestyksessä:

| Lupaus | Mitä se tarkoittaa käytännössä |
| --- | --- |
| **Työsi ei koskaan katoa** | Jokainen muutos kirjataan palautuslokiin ennen kuin se lasketaan tehdyksi. Kaatuminen maksaa korkeintaan viimeisen sekunnin. Taulutiedostoja ei koskaan kirjoiteta paikan päällä yli. |
| **Muisti noudattaa budjettia, ei taulun kokoa** | Jokaisella välimuistilla on raja, ja pisimpään käyttämättä olleet kuvat poistetaan. Valitset itse tasapainon zoomausnopeuden ja muistin välillä. |
| **Käyttöliittymä ei koskaan odota** | Dekoodaus, levytoiminnot ja pakkaus tehdään pääsäikeen ulkopuolella. Joutilas taulu käyttää 0 % suorittimesta ja näytönohjaimesta. |

Taulut tallennetaan `.moth`-tiedostoihin. Muoto on [dokumentoitu](docs/FORMAT.md), se säilyttää kuvien alkuperäiset tavut koskemattomina (ei uudelleenpakkausta) ja tallentaa identtiset kuvat vain kerran.

<a id="features"></a>

## Ominaisuudet

Tämä toimii jo nykyisessä alfassa:

**Kangas**
- Näytönohjaimella piirretty kangas `wgpu`:n avulla (Vulkan, Metal, DirectX 12) ja ohjelmistorenderöinti varalla (`--software`)
- Pehmeä zoomaus osoittimen kohtaan, kosketusvieritys hitaudella, nipistyszoomaus ja kosketuslevyn eleet
- Taulujen välilehdet ja aloitusnäkymä taulukirjastolla
- Minikartta, muokattava zoomaustaso, valinnan koko ja muistinkäyttö tilarivillä
- Animaatiot ja asetus liikkeen vähentämiseen

**Kuvat ja muistiinpanot**
- Liitä (<kbd>Ctrl</kbd>+<kbd>V</kbd>), raahaa tai avaa PNG-, JPEG-, WebP-, GIF- ja BMP-tiedostoja
- Tekstimuistiinpanot: kaksoisnapsauta tyhjää kangasta luodaksesi, kaksoisnapsauta muistiinpanoa muokataksesi
- Kierto, peilaus, rajaus, skaalaus, läpinäkyvyys ja harmaasävy, kaikki kumottavissa

**Järjestely**
- Siirto, monistus Alt-raahauksella, hienosäätö nuolinäppäimillä
- Tasaus, jakaminen, pinoaminen, tasojärjestys ja erikokoisten kuvien optimaalinen pakkaus
- Sovita koko taulu tai valinta näkyviin yhdellä <kbd>Space</kbd>-painalluksella

**Yhteensopivuus**
- Avaa **BeeRef**-taulut `.bee`: kuvat, tekstimuistiinpanot, kierto, peilaus ja skaalaus
- Avaa **PureRef**-taulut `.pur` (1.10 ja 2.x): upotetut kuvat ja tavalliset muistiinpanot; tukemattomista kohteista ilmoitetaan, eikä niitä pudoteta hiljaa pois

**Vienti**
- Taulut, valinnat tai rajatut alueet PNG-, SVG- ja PDF-muotoon
- Alkuperäiset kuvat täsmälleen sellaisina kuin ne lisättiin, yksitellen tai ZIP-pakettina
- Väripipetti, joka kopioi HEX-, RGB- tai HSV-arvon

**Nopeus ja muisti**
- PureRefin kaltaiset suorituskykytilat: *In memory* (oletus), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Jokainen kuva dekoodataan kerran zoomausaluetta kohden; myöhemmät zoomausaskeleet käyttävät välimuistissa olevia tarkkuustasoja
- RAM-välimuisti on rajattu 25 %:iin käytettävissä olevasta muistista (enintään 4 GiB), levyvälimuisti 2 GiB:hen

**Omanlaiseksesi**
- Kaikki näppäimistö- ja hiirikomennot ovat muokattavissa, ristiriidat näytetään etukäteen
- BeeRef- ja PureRef-tyyliset navigointiesiasetukset
- Komentopaletti (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Käyttöliittymä englanniksi ja venäjäksi ([lisää oma kielesi](docs/LOCALIZATION.md))

<details>
<summary><b>Taulukirjasto (kuvakaappaus)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="MothRefin taulukirjasto ensimmäisellä käynnistyskerralla" width="900">
</details>

<a id="download"></a>

## Lataa

Lataa uusin versio **[GitHub Releasesista](https://github.com/hlophlopgaming/mothRef/releases)**.

| Alusta | Tiedosto | Käynnistys |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Pura ja käynnistä `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Pura ja käynnistä `mothref` Päätteestä |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Pura ja käynnistä `mothref` Päätteestä |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Pura ja käynnistä `./mothref` (Ubuntu 24.04 tai uudempi, tai yhteensopiva) |

Ohjelmatiedostoja ei ole allekirjoitettu, eikä macOS-versioita ole vielä notarisoitu, joten järjestelmä voi pyytää vahvistusta ensimmäisellä käynnistyskerralla. Jokaisen julkaisun mukana tulevat SHA-256-tarkistussummat.

Asennusohjelmat ja paketinhallinnat (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR ja muut) on [suunniteltu](ROADMAP.md#distribution) versiosta 0.3 alkaen.
Myös **Android**-sovellus ja **verkkoversio** on suunnitteilla; ydin kirjoitetaan jo nyt niin, että se on siirrettävissä molemmille.

<a id="getting-around"></a>

## Ohjaus

| Toiminto | Näin |
| --- | --- |
| Lisää kuvia | Raahaa tiedostoja, <kbd>Ctrl</kbd>+<kbd>V</kbd> tai **Paste image** |
| Panoroi | Raahaa keskipainikkeella, <kbd>Space</kbd>+raahaus tai vasemmalla painikkeella Käsi-työkalulla (<kbd>H</kbd>) |
| Zoomaa | Hiiren rulla, nipistys tai painikkeet <kbd>+</kbd> / <kbd>−</kbd> |
| Sovita kaikki | <kbd>Home</kbd> tai paina <kbd>Space</kbd>, kun mitään ei ole valittu |
| Sovita valinta | Paina <kbd>Space</kbd> tai kaksoisnapsauta valittua kuvaa (uudelleen palataksesi) |
| Monista raahatessa | <kbd>Alt</kbd>+raahaus |
| Kierrä / peilaa | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Komentopaletti | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Muokkaa pikanäppäimiä | <kbd>/</kbd> |

Käytä macOS:ssä <kbd>Cmd</kbd> <kbd>Ctrl</kbd>:n sijaan. Koko luettelo ja vertailu PureRefiin ovat tiedostossa [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (englanniksi).

<details>
<summary><b>Tunnetut rajoitukset</b></summary>

- Tiedostojen raahaaminen natiivissa Waylandissa ei vielä toimi (winit 0.30:n rajoitus). Käytä X11:tä/XWaylandia, anna kuvat komentorivillä tai liitä ne.
- Taulut tukevat enintään 10 000 kohdetta; esikatselut näyttävät enintään 2048 px tarkkuutta kuvaa kohden. Täyden resoluution ruutupohjainen zoomaus on tiekartalla.
- PureRefin linkkejä, muotoiltuja muistiinpanoja, piirroksia ja osaa rajauksista ei vielä tuoda.

</details>

<a id="roadmap"></a>

## Tiekartta

| Julkaisu | Virstanpylväs | Lupaus | Tila |
| --- | --- | --- | --- |
| **0.1** esialfa | M1 | Ei hukkaa tietoja | alfaversiot julkaistu |
| **0.3** julkinen alfa | M2, M2.5 | Korvaa BeeRefin, avaa `.bee`- ja `.pur`-tiedostot, zoomaa PureRefiä nopeammin | työn alla |
| **0.7** beta | M3, M4 | Päällystila, selainlaajennus, PureRef 2:n ominaisuudet | suunniteltu |
| **1.0** | M5–M7 | Taulukirjasto pinoilla ja tunnisteilla, Milanote-tyyliset elementit, kaikki suorituskykytavoitteet | suunniteltu |
| **2.0** | M8, M9 | Verkkoversio ja reaaliaikainen yhteistyö | myöhemmin |
| — | — | Android-sovellus | suunniteltu |

Version 1.0 jälkeen: työkaluja taiteilijoille, arviointi- ja esitystila, Obsidian/Markdown-yhteensopivuus ja komentorivityökalu.
Koko suunnitelma, ominaisuuslistat ja suorituskykytavoitteet löytyvät tiedostosta [ROADMAP.md](ROADMAP.md) (englanniksi).

<a id="build-from-source"></a>

## Kääntäminen lähdekoodista

Tarvitset [Rustin](https://rustup.rs) version 1.95 tai uudemman. Linuxissa asenna myös X11/Wayland-otsikkotiedostot (`libxkbcommon-dev` ja `libwayland-dev` Debianissa/Ubuntussa).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Käytä aina `--release`-lippua: kuvadekooderit tarvitsevat optimointeja, ja debug-käännökset ovat paljon hitaampia.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # avaa kuvat käynnistyksessä
cargo run -p mothref --release -- board.moth                # avaa taulu
cargo run -p mothref --release -- --software                # pakota suoritinrenderöinti
```

<details>
<summary><b>Tarkistukset, suorituskykytestit ja fuzzaus</b></summary>

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

`xtask` käyttää optimoituja käännöksiä, jotta mittauksiin ei tule debug-kuormaa. Käytä `bench --quick --enforce` referenssilaitteistolla 8 ms:n kehysbudjetin (p99) vaatimiseen ja `bench --quick --software` suoritinsovittimen testaamiseen.

Dekooderien fuzzaukseen asenna `cargo-fuzz` ja nightly-työkaluketju:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

Fuzzauskohteet ovat myös `moth_reader`, `bee_reader` ja `pur_reader`.

</details>

<details>
<summary><b>Projektin rakenne</b></summary>

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

## Dokumentaatio

- [Arkkitehtuuri](docs/ARCHITECTURE.md), [suunnittelupäätökset](docs/decisions/)
- [`.moth`-tiedostomuoto](docs/FORMAT.md)
- [Ohjaus ja pikanäppäinten vertailu PureRefiin](docs/SHORTCUTS.md)
- [Huomioita PureRef-tuonnista](docs/PUR-IMPORT.md)
- [Latauksen suorituskyky](docs/LOAD-PERFORMANCE.md)
- [Kielen lisääminen](docs/LOCALIZATION.md)
- [Julkaisuprosessi](docs/RELEASING.md), [muutosloki](CHANGELOG.md)

Dokumentaatio on englanniksi.

<a id="contributing"></a>

## Osallistuminen

Vikailmoitukset ja ideat ovat tervetulleita [Issuesiin](https://github.com/hlophlopgaming/mothRef/issues). Lue [AGENTS.md](AGENTS.md) ennen koodin lähettämistä: siinä ovat arkkitehtuurisäännöt, tietoturvasäännöt ja ajettavat tarkistukset. Se koskee sekä ihmisiä että tekoälyagentteja.

<a id="license-and-credits"></a>

## Lisenssi ja kiitokset

MothRef on vapaa ohjelmisto, jonka lisenssi on **[GNU GPL v3.0 tai uudempi](LICENSE)**. Kolmansien osapuolten fontit ja koodi on lueteltu tiedostossa [THIRD_PARTY.md](THIRD_PARTY.md).

PureRef-tuonti perustuu LazyRefin [`purcore`](https://github.com/achrllrogia45/LazyRef)-kirjaston haaraan (MIT). Nimet ”BeeRef”, ”PureRef” ja ”Milanote” mainitaan vain yhteensopivuuden kuvaamiseksi; MothRef ei ole sidoksissa niihin.

Kuvakaappauksen kuvitukset ovat [David Revoyn](https://www.davidrevoy.com) tekemiä: [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) ja [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), lisenssi [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Kuvituksia on pienennetty näyttämistä varten.

<div align="center">
<br>
<sub>Tehty Rustilla ja suurella määrällä koiperhosia.</sub>
</div>
