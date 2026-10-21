<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="MothRef logosu: piksel sanatıyla bir güve" width="128" height="128">
</picture>

# MothRef

**Sanatçılar için hızlı ve çökmeye dayanıklı bir referans panosu.**<br>
Görselleri sonsuz bir tuvale bırakın, düzenleyin ve çalışırken gözünüzün önünde tutun.

[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**İndir**](#download) · [Özellikler](#features) · [Kontroller](#getting-around) · [Yol haritası](#roadmap) · [Derleme](#build-from-source) · [Belgeler](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <b>Türkçe</b> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="Koyu bir tuvalde üç illüstrasyon bulunan bir MothRef panosu" width="900">

</div>

> [!WARNING]
> MothRef **erken alfa** aşamasında (`0.1.0-alpha`). Kaydetme, kurtarma ve geri alma çalışıyor, ancak uzun güvenilirlik testleri henüz tamamlanmadı. Önemli panolarınızın ve orijinal görsellerin ayrı kopyalarını saklayın.

---

<a id="why-mothref"></a>

## Neden MothRef

[BeeRef](https://github.com/rbreu/beeref) harika bir açık kaynak referans panosuydu, ancak geliştirilmesi durdu. [PureRef](https://www.pureref.com) hızlı ve özenli, ancak kapalı kaynak. MothRef, BeeRef'in bıraktığı yerden devam ediyor: yerel, açık ve RAM'inizi tüketmeden devasa panoları kaldıracak şekilde tasarlandı.

Bu sırayla üç söz üzerine kurulu:

| Söz | Pratikte anlamı |
| --- | --- |
| **Emeğiniz asla kaybolmaz** | Her değişiklik, tamamlanmış sayılmadan önce bir kurtarma günlüğüne yazılır. Bir çökme en fazla son saniyeye mal olur. Pano dosyalarının üzerine asla yerinde yazılmaz. |
| **Bellek pano boyutuna değil, bütçeye uyar** | Her önbelleğin bir sınırı vardır ve en uzun süredir kullanılmayan görselleri çıkarır. Yakınlaştırma hızı ile bellek arasındaki dengeyi siz seçersiniz. |
| **Arayüz asla beklemez** | Kod çözme, disk işlemleri ve sıkıştırma ana iş parçacığının dışında çalışır. Boşta duran bir pano %0 CPU ve GPU kullanır. |

Panolar `.moth` uzantısını kullanır. Format [belgelenmiştir](docs/FORMAT.md), görsellerin orijinal baytlarını olduğu gibi saklar (yeniden kodlama yok) ve aynı görselleri tekrar saklamaz.

<a id="features"></a>

## Özellikler

Mevcut alfada şimdiden çalışanlar:

**Tuval**
- `wgpu` ile GPU tuvali (Vulkan, Metal, DirectX 12) ve yedek olarak yazılımsal işleme (`--software`)
- İmlece doğru akıcı yakınlaştırma, ataletli dokunmatik kaydırma, sıkıştırma ve dokunmatik yüzey hareketleri
- Pano sekmeleri ve pano kitaplığı içeren bir başlangıç ekranı
- Mini harita, düzenlenebilir yakınlaştırma düzeyi, durum çubuğunda seçim boyutu ve bellek kullanımı
- Hareketi azaltma ayarıyla animasyonlar

**Görseller ve notlar**
- PNG, JPEG, WebP, GIF ve BMP dosyalarını yapıştırın (<kbd>Ctrl</kbd>+<kbd>V</kbd>), sürükleyin veya açın
- Metin notları: boş tuvale çift tıklayarak oluşturun, bir nota çift tıklayarak düzenleyin
- Döndürme, çevirme, kırpma, ölçekleme, opaklık ve gri tonlama; hepsi geri alınabilir

**Düzenleme**
- Taşıma, Alt ile sürükleyerek çoğaltma, ok tuşlarıyla ince ayar
- Hizalama, dağıtma, yığma, katman sırası ve farklı boyutlu görsellerin en iyi şekilde yerleştirilmesi
- Tüm panoyu veya seçimi tek bir <kbd>Space</kbd> dokunuşuyla ekrana sığdırma

**Uyumluluk**
- **BeeRef** `.bee` panolarını açar: görseller, metin notları, döndürme, çevirme ve ölçek
- **PureRef** `.pur` panolarını açar (1.10 ve 2.x): gömülü görseller ve düz notlar; desteklenmeyen öğeler bildirilir, asla sessizce atılmaz

**Dışa aktarma**
- Panoları, seçimleri veya kırpılmış alanları PNG, SVG ve PDF olarak
- Orijinal görselleri eklendikleri haliyle, tek tek veya ZIP olarak
- HEX, RGB veya HSV kopyalayan renk seçici

**Hız ve bellek**
- PureRef tarzı performans modları: *In memory* (varsayılan), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Her görselin kodu yakınlaştırma aralığı başına bir kez çözülür; sonraki adımlar önbellekteki ayrıntı düzeylerini yeniden kullanır
- RAM önbelleği kullanılabilir belleğin %25'iyle (en fazla 4 GiB), disk önbelleği 2 GiB ile sınırlı

**Size göre**
- Tüm klavye ve fare kısayolları düzenlenebilir, çakışmalar önceden gösterilir
- BeeRef tarzı ve PureRef tarzı gezinme ön ayarları
- Komut paleti (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- İngilizce ve Rusça arayüz ([kendi dilinizi ekleyin](docs/LOCALIZATION.md))

<details>
<summary><b>Pano kitaplığı (ekran görüntüsü)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="İlk açılışta MothRef pano kitaplığı" width="900">
</details>

<a id="download"></a>

## İndir

En son sürümü **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)** sayfasından indirin.

| Platform | Dosya | Nasıl çalıştırılır |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Arşivden çıkarın ve `mothref.exe` dosyasını çalıştırın |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Arşivden çıkarın ve `mothref` dosyasını Terminal'den çalıştırın |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Arşivden çıkarın ve `mothref` dosyasını Terminal'den çalıştırın |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Arşivden çıkarın ve `./mothref` çalıştırın (Ubuntu 24.04 veya daha yenisi ya da uyumlu bir dağıtım) |

Dosyalar imzalı değil ve macOS sürümleri henüz noter onaylı değil, bu yüzden sisteminiz ilk açılışta onay isteyebilir. Her sürümle birlikte SHA-256 sağlama toplamları yayımlanır.

Yükleyiciler ve paket yöneticileri (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR ve diğerleri) 0.3'ten itibaren [planlanıyor](ROADMAP.md#distribution).
Bir **Android** uygulaması ve **web** sürümü de planlanıyor; çekirdek şimdiden ikisine de taşınabilecek şekilde yazılıyor.

<a id="getting-around"></a>

## Kontroller

| Yapmak istediğiniz | Kullanın |
| --- | --- |
| Görsel ekleme | Dosyaları sürükleyin, <kbd>Ctrl</kbd>+<kbd>V</kbd> veya **Paste image** |
| Kaydırma | Orta tuşla sürükleme, <kbd>Space</kbd>+sürükleme ya da El aracıyla (<kbd>H</kbd>) sol tuşla sürükleme |
| Yakınlaştırma | Fare tekerleği, sıkıştırma veya <kbd>+</kbd> / <kbd>−</kbd> düğmeleri |
| Hepsini sığdır | <kbd>Home</kbd> veya hiçbir şey seçili değilken <kbd>Space</kbd>'e dokunun |
| Seçimi sığdır | <kbd>Space</kbd>'e dokunun veya seçili bir görsele çift tıklayın (geri dönmek için tekrar) |
| Sürüklerken çoğaltma | <kbd>Alt</kbd>+sürükleme |
| Döndürme / çevirme | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Komut paleti | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Kısayolları düzenleme | <kbd>/</kbd> |

macOS'ta <kbd>Ctrl</kbd> yerine <kbd>Cmd</kbd> kullanın. Tam liste ve PureRef ile karşılaştırma [docs/SHORTCUTS.md](docs/SHORTCUTS.md) dosyasında (İngilizce).

<details>
<summary><b>Bilinen sınırlamalar</b></summary>

- Yerel Wayland'de dosya sürükleyip bırakma henüz kullanılamıyor (winit 0.30 sınırlaması). X11/XWayland kullanın, görselleri komut satırından verin veya yapıştırın.
- Panolar en fazla 10.000 öğeyi destekler; önizlemeler görsel başına en fazla 2048 piksel ayrıntı gösterir. Döşemelerle tam çözünürlüklü yakınlaştırma yol haritasında.
- PureRef bağlantıları, biçimli notlar, çizimler ve bazı kırpmalar henüz içe aktarılmıyor.

</details>

<a id="roadmap"></a>

## Yol haritası

| Sürüm | Kilometre taşı | Söz | Durum |
| --- | --- | --- | --- |
| **0.1** ön alfa | M1 | Veri kaybetmez | alfa sürümleri yayında |
| **0.3** genel alfa | M2, M2.5 | BeeRef'in yerini alır, `.bee` ve `.pur` açar, PureRef'ten hızlı yakınlaştırır | sürüyor |
| **0.7** beta | M3, M4 | Kaplama modu, tarayıcı eklentisi, PureRef 2 ile özellik eşitliği | planlandı |
| **1.0** | M5–M7 | Yığınlar ve etiketlerle pano kitaplığı, Milanote tarzı öğeler, tüm performans hedefleri | planlandı |
| **2.0** | M8, M9 | Web sürümü ve gerçek zamanlı ortak çalışma | daha sonra |
| — | — | Android uygulaması | planlandı |

1.0'dan sonra: sanatçı araçları, inceleme ve sunum modu, Obsidian/Markdown entegrasyonu ve bir komut satırı aracı.
Tam plan, özellik listeleri ve performans hedefleri [ROADMAP.md](ROADMAP.md) dosyasında (İngilizce).

<a id="build-from-source"></a>

## Kaynaktan derleme

[Rust](https://rustup.rs) 1.95 veya daha yenisi gerekir. Linux'ta ayrıca X11/Wayland başlık dosyalarını kurun (Debian/Ubuntu'da `libxkbcommon-dev` ve `libwayland-dev`).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Her zaman `--release` kullanın: görsel kod çözücüler optimizasyonlara dayanır ve debug derlemeleri çok daha yavaştır.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # açılışta görselleri aç
cargo run -p mothref --release -- board.moth                # bir pano aç
cargo run -p mothref --release -- --software                # CPU ile işlemeyi zorla
```

<details>
<summary><b>Denetimler, kıyaslamalar ve fuzzing</b></summary>

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

`xtask`, ölçümlere debug yükü karışmasın diye optimize derlemeler kullanır. Referans donanımda 8 ms kare bütçesini (p99) zorunlu kılmak için `bench --quick --enforce`, CPU bağdaştırıcısını denemek için `bench --quick --software` kullanın.

Kod çözücüleri fuzz etmek için `cargo-fuzz` ve bir nightly araç zinciri kurun:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

`moth_reader`, `bee_reader` ve `pur_reader` için de fuzz hedefleri vardır.

</details>

<details>
<summary><b>Proje yapısı</b></summary>

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

## Belgeler

- [Mimari](docs/ARCHITECTURE.md), [tasarım kararları](docs/decisions/)
- [`.moth` dosya formatı](docs/FORMAT.md)
- [Kontroller ve PureRef kısayol karşılaştırması](docs/SHORTCUTS.md)
- [PureRef içe aktarma notları](docs/PUR-IMPORT.md)
- [Yükleme performansı](docs/LOAD-PERFORMANCE.md)
- [Dil ekleme](docs/LOCALIZATION.md)
- [Sürüm süreci](docs/RELEASING.md), [değişiklik günlüğü](CHANGELOG.md)

Belgeler İngilizcedir.

<a id="contributing"></a>

## Katkıda bulunma

Hata bildirimleri ve fikirler [Issues](https://github.com/hlophlopgaming/mothRef/issues) bölümüne bekleniyor. Kod göndermeden önce [AGENTS.md](AGENTS.md) dosyasını okuyun: mimari kuralları, veri güvenliği kurallarını ve hangi denetimlerin çalıştırılacağını anlatır. İnsanlar ve yapay zekâ ajanları için aynı şekilde geçerlidir.

<a id="license-and-credits"></a>

## Lisans ve teşekkürler

MothRef, **[GNU GPL v3.0 veya sonrası](LICENSE)** altında özgür bir yazılımdır. Üçüncü taraf yazı tipleri ve kodlar [THIRD_PARTY.md](THIRD_PARTY.md) dosyasında listelenir.

PureRef içe aktarma, LazyRef'teki [`purcore`](https://github.com/achrllrogia45/LazyRef) kütüphanesinin bir çatalına (MIT) dayanır. "BeeRef", "PureRef" ve "Milanote" yalnızca uyumluluğu açıklamak için anılır; MothRef bunlarla bağlantılı değildir.

Ekran görüntüsündeki illüstrasyonlar [David Revoy](https://www.davidrevoy.com)'a aittir: [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) ve [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) lisanslı. İllüstrasyonlar gösterim için küçültülmüştür.

<div align="center">
<br>
<sub>Rust ve bolca güveyle yapıldı.</sub>
</div>
