<div dir="rtl">

<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="شعار MothRef: عثة بأسلوب البكسل" width="128" height="128">
</picture>

# MothRef

**لوحة مراجع سريعة ومقاومة للأعطال للفنانين.**<br>
أفلت الصور على لوحة لا نهائية، ورتّبها، وأبقها أمام عينيك أثناء العمل.

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**التنزيل**](#download) · [الميزات](#features) · [التحكم](#getting-around) · [خارطة الطريق](#roadmap) · [البناء من المصدر](#build-from-source) · [التوثيق](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <b>العربية</b> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="لوحة MothRef تحتوي على ثلاثة رسوم على خلفية داكنة" width="900">

</div>

> [!WARNING]
> MothRef في **مرحلة ألفا مبكرة** (`0.1.0-alpha`). الحفظ والاستعادة والتراجع تعمل، لكن اختبارات الموثوقية الطويلة لم تكتمل بعد. احتفظ بنسخ منفصلة من لوحاتك المهمة والصور الأصلية.

---

<a id="why-mothref"></a>

## لماذا MothRef

كان [BeeRef](https://github.com/rbreu/beeref) لوحة مراجع رائعة ومفتوحة المصدر، لكن تطويره توقف. و[PureRef](https://www.pureref.com) سريع ومتقن، لكنه مغلق المصدر. يكمل MothRef من حيث توقف BeeRef: تطبيق أصلي ومفتوح، مصمم للوحات الضخمة دون أن يستهلك ذاكرة جهازك.

يقوم على ثلاثة وعود، بهذا الترتيب:

| الوعد | ماذا يعني عمليًا |
| --- | --- |
| **لن تفقد عملك أبدًا** | يُسجَّل كل تغيير في سجل استعادة قبل اعتباره منجزًا. أي عطل مفاجئ يكلّفك آخر ثانية على الأكثر. لا يُكتب فوق ملفات اللوحات في مكانها أبدًا. |
| **الذاكرة تتبع ميزانية، لا حجم اللوحة** | لكل ذاكرة تخزين مؤقت حد أقصى، وتُزال منها الصور الأقل استخدامًا مؤخرًا. أنت من يختار التوازن بين سرعة التكبير واستهلاك الذاكرة. |
| **الواجهة لا تنتظر أبدًا** | فك الترميز والقرص والضغط تعمل خارج الخيط الرئيسي. اللوحة الخاملة تستهلك 0% من المعالج ومعالج الرسوميات. |

تُحفظ اللوحات بامتداد `.moth`. الصيغة [موثّقة](docs/FORMAT.md)، وتحتفظ ببايتات صورك الأصلية كما هي (دون إعادة ترميز) ولا تكرر الصور المتطابقة.

<a id="features"></a>

## الميزات

ما يعمل بالفعل في نسخة ألفا الحالية:

**اللوحة**
- لوحة تعمل على معالج الرسوميات عبر `wgpu` (Vulkan وMetal وDirectX 12) مع عرض برمجي احتياطي (`--software`)
- تكبير سلس نحو المؤشر، وتمرير باللمس مع قصور ذاتي، وتكبير بالقرص، وإيماءات لوحة اللمس
- ألسنة للوحات وشاشة بداية تضم مكتبة اللوحات
- خريطة مصغّرة، ومستوى تكبير قابل للتعديل، وحجم التحديد واستهلاك الذاكرة في شريط الحالة
- حركات انتقالية مع خيار تقليل الحركة

**الصور والملاحظات**
- الصق (<kbd>Ctrl</kbd>+<kbd>V</kbd>) أو اسحب أو افتح ملفات PNG وJPEG وWebP وGIF وBMP
- ملاحظات نصية: انقر نقرًا مزدوجًا على مساحة فارغة لإنشاء ملاحظة، وعلى ملاحظة لتعديلها
- التدوير والقلب والقص وتغيير الحجم والشفافية والتدرج الرمادي، وكلها قابلة للتراجع

**الترتيب**
- النقل، والنسخ بالسحب مع Alt، والتحريك الدقيق بالأسهم
- المحاذاة والتوزيع والتكديس وترتيب الطبقات والرصّ الأمثل للصور مختلفة الأحجام
- عرض اللوحة كاملة أو التحديد الحالي بضغطة على <kbd>Space</kbd>

**التوافق**
- يفتح لوحات **BeeRef** بامتداد `.bee`: الصور والملاحظات النصية والتدوير والقلب والحجم
- يفتح لوحات **PureRef** بامتداد `.pur` (الإصداران 1.10 و2.x): الصور المضمنة والملاحظات البسيطة؛ يُبلَّغ عن العناصر غير المدعومة ولا تُحذف بصمت أبدًا

**التصدير**
- تصدير اللوحات أو التحديدات أو المناطق المقصوصة إلى PNG وSVG وPDF
- الصور الأصلية تمامًا كما أُضيفت، كلٌّ على حدة أو في ملف ZIP
- منتقي ألوان ينسخ قيم HEX أو RGB أو HSV

**السرعة والذاكرة**
- أوضاع أداء على طريقة PureRef: *In memory* (الافتراضي)، *In memory, low zoom quality*، *Cache on disk*، *No cache*
- تُفك كل صورة مرة واحدة لكل نطاق تكبير؛ وتعيد الخطوات اللاحقة استخدام مستويات التفاصيل المخزّنة
- ذاكرة التخزين في RAM محدودة بـ25% من الذاكرة المتاحة (بحد أقصى 4 GiB)، وعلى القرص بـ2 GiB

**على طريقتك**
- كل اختصارات لوحة المفاتيح والفأرة قابلة للتعديل، مع معاينة التعارضات
- إعدادات تنقّل جاهزة بأسلوب BeeRef وأسلوب PureRef
- لوحة الأوامر (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- الواجهة بالإنجليزية والروسية ([أضف لغتك](docs/LOCALIZATION.md))

<details>
<summary><b>مكتبة اللوحات (لقطة شاشة)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="مكتبة لوحات MothRef عند التشغيل الأول" width="900">
</details>

<a id="download"></a>

## التنزيل

نزّل أحدث إصدار من **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| المنصة | الملف | طريقة التشغيل |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | فك الضغط وشغّل `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | فك الضغط وشغّل `mothref` من الطرفية |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | فك الضغط وشغّل `mothref` من الطرفية |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | فك الضغط وشغّل `./mothref` (Ubuntu 24.04 أو أحدث، أو توزيعة متوافقة) |

الملفات التنفيذية غير موقّعة، وإصدارات macOS غير موثّقة بعد، لذا قد يطلب نظامك تأكيدًا عند التشغيل الأول. تُنشر مجاميع تحقق SHA-256 مع كل إصدار.

برامج التثبيت ومديرو الحزم (MSI و`.dmg` وAppImage وFlatpak وHomebrew وAUR وغيرها) [مخطط لها](ROADMAP.md#distribution) بدءًا من الإصدار 0.3.
من المخطط أيضًا تطبيق لـ**Android** ونسخة **ويب**؛ والنواة مكتوبة من الآن بحيث يسهل نقلها إلى كليهما.

<a id="getting-around"></a>

## التحكم

| لفعل هذا | استخدم |
| --- | --- |
| إضافة صور | اسحب الملفات، أو <kbd>Ctrl</kbd>+<kbd>V</kbd>، أو **Paste image** |
| تحريك العرض | السحب بالزر الأوسط، أو <kbd>Space</kbd>+سحب، أو السحب بالزر الأيسر مع أداة اليد (<kbd>H</kbd>) |
| التكبير | عجلة الفأرة، أو القرص، أو زرّا <kbd>+</kbd> / <kbd>−</kbd> |
| عرض كل شيء | <kbd>Home</kbd>، أو اضغط <kbd>Space</kbd> دون تحديد |
| عرض التحديد | اضغط <kbd>Space</kbd>، أو انقر نقرًا مزدوجًا على صورة محددة (مرة أخرى للعودة) |
| النسخ أثناء السحب | <kbd>Alt</kbd>+سحب |
| التدوير / القلب | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| لوحة الأوامر | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| تعديل الاختصارات | <kbd>/</kbd> |

على macOS استخدم <kbd>Cmd</kbd> بدلًا من <kbd>Ctrl</kbd>. القائمة الكاملة ومقارنة مع PureRef موجودة في [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (بالإنجليزية).

<details>
<summary><b>قيود معروفة</b></summary>

- سحب الملفات وإفلاتها في Wayland الأصلي غير متاح بعد (قيد في winit 0.30). استخدم X11/XWayland، أو مرّر الصور عبر سطر الأوامر، أو الصقها.
- تدعم اللوحات حتى 10,000 عنصر؛ وتعرض المعاينات حتى 2048 بكسل من التفاصيل لكل صورة. التكبير بالدقة الكاملة عبر البلاطات مدرج في خارطة الطريق.
- لا تُستورد بعد روابط PureRef والملاحظات المنسقة والرسومات وبعض عمليات القص.

</details>

<a id="roadmap"></a>

## خارطة الطريق

| الإصدار | المرحلة | الوعد | الحالة |
| --- | --- | --- | --- |
| **0.1** ما قبل ألفا | M1 | لا يفقد البيانات | إصدارات ألفا متاحة |
| **0.3** ألفا عامة | M2, M2.5 | يحل محل BeeRef، ويفتح `.bee` و`.pur`، ويكبّر أسرع من PureRef | قيد العمل |
| **0.7** بيتا | M3, M4 | وضع التراكب، وإضافة للمتصفح، وكل ميزات PureRef 2 | مخطط له |
| **1.0** | M5–M7 | مكتبة لوحات بالأكوام والوسوم، وعناصر بأسلوب Milanote، وتحقيق كل أهداف الأداء | مخطط له |
| **2.0** | M8, M9 | نسخة ويب وتعاون في الوقت الفعلي | لاحقًا |
| — | — | تطبيق Android | مخطط له |

بعد 1.0: أدوات للفنانين، ووضع المراجعة والعرض، والتكامل مع Obsidian/Markdown، وأداة لسطر الأوامر.
الخطة الكاملة وقوائم الميزات وأهداف الأداء موجودة في [ROADMAP.md](ROADMAP.md) (بالإنجليزية).

<a id="build-from-source"></a>

## البناء من المصدر

تحتاج إلى [Rust](https://rustup.rs) بالإصدار 1.95 أو أحدث. على Linux ثبّت أيضًا ترويسات X11/Wayland (`libxkbcommon-dev`, `libdbus-1-dev` و`libwayland-dev` على Debian/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

استخدم `--release` دائمًا: تعتمد مفككات ترميز الصور على التحسينات، وإصدارات التصحيح أبطأ بكثير.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # open images at startup
cargo run -p mothref --release -- board.moth                # open a board
cargo run -p mothref --release -- --software                # force the CPU renderer
```

<details>
<summary><b>الفحوص والقياسات واختبارات fuzzing</b></summary>

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

يستخدم `xtask` إصدارات محسّنة كي لا تتضمن القياسات عبء التصحيح. استخدم `bench --quick --enforce` على العتاد المرجعي لفرض ميزانية 8 ملّي ثانية للإطار (p99)، و`bench --quick --software` لتجربة محوّل يعمل على المعالج.

لاختبار fuzzing لمفككات الترميز، ثبّت `cargo-fuzz` وسلسلة أدوات nightly:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

توجد أيضًا أهداف fuzzing لـ`moth_reader` و`bee_reader` و`pur_reader`.

</details>

<details>
<summary><b>بنية المشروع</b></summary>

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

## التوثيق

- [البنية المعمارية](docs/ARCHITECTURE.md), [قرارات التصميم](docs/decisions/)
- [صيغة ملفات `.moth`](docs/FORMAT.md)
- [التحكم ومقارنة الاختصارات مع PureRef](docs/SHORTCUTS.md)
- [ملاحظات حول الاستيراد من PureRef](docs/PUR-IMPORT.md)
- [أداء التحميل](docs/LOAD-PERFORMANCE.md)
- [إضافة لغة](docs/LOCALIZATION.md)
- [عملية الإصدار](docs/RELEASING.md), [سجل التغييرات](CHANGELOG.md)

التوثيق باللغة الإنجليزية.

<a id="contributing"></a>

## المساهمة

نرحّب بتقارير الأخطاء والأفكار في [Issues](https://github.com/hlophlopgaming/mothRef/issues). قبل إرسال أي شيفرة، اقرأ [AGENTS.md](AGENTS.md): فهو يشرح قواعد البنية المعمارية وقواعد سلامة البيانات والفحوص المطلوب تشغيلها. وينطبق على البشر ووكلاء الذكاء الاصطناعي على حد سواء.

<a id="license-and-credits"></a>

## الترخيص والشكر

MothRef برنامج حر مرخّص بموجب **[GNU GPL الإصدار 3.0 أو أحدث](LICENSE)**. الخطوط والشيفرات الخارجية مدرجة في [THIRD_PARTY.md](THIRD_PARTY.md).

يعتمد الاستيراد من PureRef على نسخة متفرعة من [`purcore`](https://github.com/achrllrogia45/LazyRef) من مشروع LazyRef (MIT). تُذكر أسماء «BeeRef» و«PureRef» و«Milanote» لوصف التوافق فقط؛ وMothRef غير تابع لها.

الرسوم في لقطة الشاشة من أعمال [David Revoy](https://www.davidrevoy.com): [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html) و[Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) و[Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland)، بترخيص [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). صُغّرت الرسوم لغرض العرض.

<div align="center">
<br>
<sub>صُنع بلغة Rust وبكثير من العث.</sub>
</div>

</div>
