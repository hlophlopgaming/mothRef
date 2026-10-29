<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="MothRef का लोगो: पिक्सल आर्ट में बना एक पतंगा" width="128" height="128">
</picture>

# MothRef

**कलाकारों के लिए एक तेज़ और क्रैश से सुरक्षित रेफ़रेंस बोर्ड।**<br>
तस्वीरें एक अनंत कैनवस पर छोड़ें, उन्हें सजाएँ और काम करते समय नज़र के सामने रखें।

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**डाउनलोड**](#download) · [सुविधाएँ](#features) · [नियंत्रण](#getting-around) · [रोडमैप](#roadmap) · [सोर्स से बनाएँ](#build-from-source) · [दस्तावेज़](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <b>हिन्दी</b> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="गहरे कैनवस पर तीन चित्रों वाला एक MothRef बोर्ड" width="900">

</div>

> [!WARNING]
> MothRef अभी **शुरुआती अल्फ़ा** (`0.1.0-alpha`) में है। सेव करना, रिकवरी और अनडू काम करते हैं, लेकिन लंबी विश्वसनीयता जाँच अभी बाकी है। अपने ज़रूरी बोर्ड और मूल तस्वीरों की अलग प्रतियाँ रखें।

---

<a id="why-mothref"></a>

## MothRef क्यों

[BeeRef](https://github.com/rbreu/beeref) एक बढ़िया ओपन-सोर्स रेफ़रेंस बोर्ड था, लेकिन उसका विकास रुक गया। [PureRef](https://www.pureref.com) तेज़ और निखरा हुआ है, पर बंद स्रोत है। MothRef वहीं से आगे बढ़ता है जहाँ BeeRef रुका था: नेटिव, खुला, और विशाल बोर्ड संभालने के लिए बना, बिना आपकी RAM खाए।

यह तीन वादों पर टिका है, इसी क्रम में:

| वादा | व्यवहार में इसका मतलब |
| --- | --- |
| **आपका काम कभी नहीं खोता** | हर बदलाव पूरा माने जाने से पहले रिकवरी जर्नल में लिखा जाता है। क्रैश होने पर ज़्यादा से ज़्यादा आख़िरी एक सेकंड जाता है। बोर्ड फ़ाइलों को कभी उसी जगह ओवरराइट नहीं किया जाता। |
| **मेमोरी बजट के हिसाब से चलती है, बोर्ड के आकार से नहीं** | हर कैश की एक सीमा है, और सबसे पहले वे तस्वीरें हटती हैं जिनका सबसे लंबे समय से इस्तेमाल नहीं हुआ। ज़ूम की रफ़्तार और मेमोरी के बीच संतुलन आप चुनते हैं। |
| **इंटरफ़ेस कभी इंतज़ार नहीं करता** | डिकोडिंग, डिस्क और कम्प्रेशन मुख्य थ्रेड से बाहर चलते हैं। खाली पड़ा बोर्ड 0% CPU और GPU लेता है। |

बोर्ड `.moth` एक्सटेंशन वाली फ़ाइलों में सेव होते हैं। यह फ़ॉर्मेट [दस्तावेज़ित](docs/FORMAT.md) है, आपकी तस्वीरों के मूल बाइट्स बिना री-एन्कोड किए रखता है, और एक जैसी तस्वीरें दोबारा नहीं रखता।

<a id="features"></a>

## सुविधाएँ

मौजूदा अल्फ़ा में अभी क्या काम करता है:

**कैनवस**
- `wgpu` पर GPU कैनवस (Vulkan, Metal, DirectX 12), साथ में सॉफ़्टवेयर रेंडरिंग का विकल्प (`--software`)
- पॉइंटर की ओर स्मूद ज़ूम, जड़त्व के साथ टच पैन, पिंच ज़ूम और ट्रैकपैड जेस्चर
- बोर्ड टैब और बोर्ड लाइब्रेरी वाली शुरुआती स्क्रीन
- मिनीमैप, बदलने योग्य ज़ूम स्तर, स्टेटस बार में चयन का आकार और मेमोरी उपयोग
- कम हलचल वाली सेटिंग के साथ एनिमेशन

**तस्वीरें और नोट्स**
- PNG, JPEG, WebP, GIF और BMP फ़ाइलें पेस्ट करें (<kbd>Ctrl</kbd>+<kbd>V</kbd>), खींचकर छोड़ें या खोलें
- टेक्स्ट नोट्स: नया नोट बनाने के लिए खाली कैनवस पर डबल-क्लिक करें, बदलने के लिए नोट पर डबल-क्लिक करें
- घुमाना, पलटना, क्रॉप, स्केल, अपारदर्शिता और ग्रेस्केल, सब अनडू किए जा सकते हैं

**सजाना**
- खिसकाना, Alt के साथ खींचकर कॉपी बनाना, तीर कुंजियों से थोड़ा-थोड़ा सरकाना
- संरेखण, बराबर बाँटना, ढेर बनाना, परतों का क्रम और अलग-अलग आकार की तस्वीरों को सबसे अच्छे ढंग से जमाना
- <kbd>Space</kbd> के एक टैप से पूरा बोर्ड या चयन स्क्रीन में फ़िट करें

**संगतता**
- **BeeRef** के `.bee` बोर्ड खोलता है: तस्वीरें, टेक्स्ट नोट्स, घुमाव, फ़्लिप और स्केल
- **PureRef** के `.pur` बोर्ड (1.10 और 2.x) खोलता है: एम्बेडेड तस्वीरें और सादे नोट्स; जो चीज़ें समर्थित नहीं हैं उनकी जानकारी दी जाती है, उन्हें चुपचाप हटाया नहीं जाता

**एक्सपोर्ट**
- बोर्ड, चयन या क्रॉप किए हिस्से PNG, SVG और PDF में
- मूल तस्वीरें ठीक वैसी ही जैसी जोड़ी गई थीं, एक-एक करके या ZIP में
- कलर पिकर जो HEX, RGB या HSV कॉपी करता है

**रफ़्तार और मेमोरी**
- PureRef जैसे परफ़ॉर्मेंस मोड: *In memory* (डिफ़ॉल्ट), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- हर तस्वीर हर ज़ूम रेंज में एक बार डिकोड होती है; आगे के ज़ूम स्टेप कैश किए गए डिटेल स्तर दोबारा इस्तेमाल करते हैं
- RAM कैश उपलब्ध मेमोरी के 25% तक सीमित (अधिकतम 4 GiB), डिस्क कैश 2 GiB तक

**अपने हिसाब से**
- हर कीबोर्ड और माउस शॉर्टकट बदला जा सकता है, टकराव पहले से दिखते हैं
- BeeRef और PureRef शैली के नेविगेशन प्रीसेट
- कमांड पैलेट (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- इंटरफ़ेस अंग्रेज़ी और रूसी में ([अपनी भाषा जोड़ें](docs/LOCALIZATION.md))

<details>
<summary><b>बोर्ड लाइब्रेरी (स्क्रीनशॉट)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="पहली बार खोलने पर MothRef की बोर्ड लाइब्रेरी" width="900">
</details>

<a id="download"></a>

## डाउनलोड

नया बिल्ड **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)** से डाउनलोड करें।

| प्लेटफ़ॉर्म | फ़ाइल | कैसे चलाएँ |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | एक्सट्रैक्ट करें और `mothref.exe` चलाएँ |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | एक्सट्रैक्ट करें और Terminal से `mothref` चलाएँ |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | एक्सट्रैक्ट करें और Terminal से `mothref` चलाएँ |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | एक्सट्रैक्ट करें और `./mothref` चलाएँ (Ubuntu 24.04 या नया, या उसके अनुकूल) |

बाइनरी पर हस्ताक्षर नहीं हैं और macOS बिल्ड अभी नोटराइज़्ड नहीं हैं, इसलिए पहली बार चलाने पर सिस्टम पुष्टि माँग सकता है। हर रिलीज़ के साथ SHA-256 चेकसम दिए जाते हैं।

इंस्टॉलर और पैकेज मैनेजर (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR और अन्य) 0.3 से [योजना में](ROADMAP.md#distribution) हैं।
**Android** ऐप और **वेब** संस्करण की भी योजना है; कोर अभी से इस तरह लिखा जा रहा है कि दोनों पर ले जाया जा सके।

<a id="getting-around"></a>

## नियंत्रण

| यह करने के लिए | इस्तेमाल करें |
| --- | --- |
| तस्वीरें जोड़ना | फ़ाइलें खींचकर छोड़ें, <kbd>Ctrl</kbd>+<kbd>V</kbd> या **Paste image** |
| पैन करना | बीच वाले बटन से खींचें, <kbd>Space</kbd>+खींचें, या हैंड टूल (<kbd>H</kbd>) के साथ बाएँ बटन से खींचें |
| ज़ूम | माउस व्हील, पिंच या <kbd>+</kbd> / <kbd>−</kbd> बटन |
| सब कुछ फ़िट करें | <kbd>Home</kbd>, या कुछ भी चुने बिना <kbd>Space</kbd> दबाएँ |
| चयन फ़िट करें | <kbd>Space</kbd> दबाएँ, या चुनी हुई तस्वीर पर डबल-क्लिक करें (वापस जाने के लिए फिर से) |
| खींचते समय कॉपी बनाना | <kbd>Alt</kbd>+खींचें |
| घुमाएँ / पलटें | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| कमांड पैलेट | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| शॉर्टकट बदलें | <kbd>/</kbd> |

macOS पर <kbd>Ctrl</kbd> की जगह <kbd>Cmd</kbd> इस्तेमाल करें। पूरी सूची और PureRef से तुलना [docs/SHORTCUTS.md](docs/SHORTCUTS.md) में है (अंग्रेज़ी में)।

<details>
<summary><b>ज्ञात सीमाएँ</b></summary>

- नेटिव Wayland में फ़ाइलें खींचकर छोड़ना अभी उपलब्ध नहीं है (winit 0.30 की सीमा)। X11/XWayland इस्तेमाल करें, कमांड लाइन से तस्वीरें दें, या पेस्ट करें।
- बोर्ड 10,000 चीज़ों तक संभालते हैं; प्रीव्यू हर तस्वीर का 2048 पिक्सल तक का डिटेल दिखाते हैं। टाइल्स के साथ पूरे रिज़ॉल्यूशन वाला ज़ूम रोडमैप में है।
- PureRef के लिंक, फ़ॉर्मैट किए नोट्स, ड्रॉइंग और कुछ क्रॉप अभी इम्पोर्ट नहीं होते।

</details>

<a id="roadmap"></a>

## रोडमैप

| रिलीज़ | पड़ाव | वादा | स्थिति |
| --- | --- | --- | --- |
| **0.1** प्री-अल्फ़ा | M1 | डेटा नहीं खोता | अल्फ़ा बिल्ड जारी |
| **0.3** पब्लिक अल्फ़ा | M2, M2.5 | BeeRef की जगह लेता है, `.bee` और `.pur` खोलता है, PureRef से तेज़ ज़ूम करता है | काम चल रहा है |
| **0.7** बीटा | M3, M4 | ओवरले मोड, PureRef 2 की सभी सुविधाएँ | योजना में |
| **1.0** | M5–M7 | स्टैक और टैग वाली बोर्ड लाइब्रेरी, Milanote जैसे एलिमेंट, सभी परफ़ॉर्मेंस लक्ष्य पूरे | योजना में |
| **2.0** | M8, M9 | वेब संस्करण और रियल-टाइम सहयोग | बाद में |
| — | — | Android ऐप | योजना में |

1.0 के बाद: कलाकारों के लिए टूल, रिव्यू और प्रेज़ेंटेशन मोड, Obsidian/Markdown के साथ जुड़ाव और एक कमांड-लाइन टूल।
पूरी योजना, सुविधाओं की सूचियाँ और परफ़ॉर्मेंस लक्ष्य [ROADMAP.md](ROADMAP.md) में हैं (अंग्रेज़ी में)।

<a id="build-from-source"></a>

## सोर्स से बनाएँ

आपको [Rust](https://rustup.rs) 1.95 या नया चाहिए। Linux पर X11/Wayland हेडर भी इंस्टॉल करें (Debian/Ubuntu पर `libxkbcommon-dev`, `libdbus-1-dev` और `libwayland-dev`)।

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

हमेशा `--release` इस्तेमाल करें: इमेज डिकोडर ऑप्टिमाइज़ेशन पर निर्भर हैं, और डीबग बिल्ड काफ़ी धीमे होते हैं।

```sh
cargo run -p mothref --release -- image.png reference.jpg   # शुरू होते ही तस्वीरें खोलें
cargo run -p mothref --release -- board.moth                # बोर्ड खोलें
cargo run -p mothref --release -- --software                # CPU रेंडरिंग अनिवार्य करें
```

<details>
<summary><b>जाँच, बेंचमार्क और फ़ज़िंग</b></summary>

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

`xtask` ऑप्टिमाइज़्ड बिल्ड इस्तेमाल करता है ताकि मापों में डीबग का अतिरिक्त भार न जुड़े। संदर्भ हार्डवेयर पर 8 ms फ़्रेम बजट (p99) लागू करने के लिए `bench --quick --enforce`, और CPU एडैप्टर जाँचने के लिए `bench --quick --software` चलाएँ।

डिकोडर की फ़ज़िंग के लिए `cargo-fuzz` और nightly टूलचेन इंस्टॉल करें:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

`moth_reader`, `bee_reader` और `pur_reader` के लिए भी फ़ज़ टारगेट मौजूद हैं।

</details>

<details>
<summary><b>प्रोजेक्ट की संरचना</b></summary>

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

## दस्तावेज़

- [आर्किटेक्चर](docs/ARCHITECTURE.md), [डिज़ाइन निर्णय](docs/decisions/)
- [`.moth` फ़ाइल फ़ॉर्मेट](docs/FORMAT.md)
- [नियंत्रण और PureRef से शॉर्टकट की तुलना](docs/SHORTCUTS.md)
- [PureRef इम्पोर्ट से जुड़े नोट्स](docs/PUR-IMPORT.md)
- [लोडिंग परफ़ॉर्मेंस](docs/LOAD-PERFORMANCE.md)
- [नई भाषा जोड़ना](docs/LOCALIZATION.md)
- [रिलीज़ प्रक्रिया](docs/RELEASING.md), [बदलावों की सूची](CHANGELOG.md)

दस्तावेज़ अंग्रेज़ी में हैं।

<a id="contributing"></a>

## योगदान

बग रिपोर्ट और सुझावों का [Issues](https://github.com/hlophlopgaming/mothRef/issues) में स्वागत है। कोड भेजने से पहले [AGENTS.md](AGENTS.md) पढ़ें: इसमें आर्किटेक्चर के नियम, डेटा सुरक्षा के नियम और चलाई जाने वाली जाँचें हैं। यह इंसानों और AI एजेंटों, दोनों पर लागू होता है।

<a id="license-and-credits"></a>

## लाइसेंस और आभार

MothRef **[GNU GPL v3.0 या बाद के संस्करण](LICENSE)** के तहत मुफ़्त सॉफ़्टवेयर है। तीसरे पक्ष के फ़ॉन्ट और कोड [THIRD_PARTY.md](THIRD_PARTY.md) में सूचीबद्ध हैं।

PureRef इम्पोर्ट LazyRef के [`purcore`](https://github.com/achrllrogia45/LazyRef) के एक फ़ोर्क (MIT) पर आधारित है। "BeeRef", "PureRef" और "Milanote" के नाम सिर्फ़ संगतता बताने के लिए दिए गए हैं; MothRef इनसे संबद्ध नहीं है।

स्क्रीनशॉट में दिख रहे चित्र [David Revoy](https://www.davidrevoy.com) के हैं: [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) और [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) लाइसेंस के तहत। चित्र दिखाने के लिए छोटे किए गए हैं।

<div align="center">
<br>
<sub>Rust और ढेर सारे पतंगों के साथ बनाया गया।</sub>
</div>
