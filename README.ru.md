<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="Пиксельный логотип MothRef — моль" width="128" height="128">
</picture>

# MothRef

**Быстрая и надёжная доска референсов для художников.**<br>
Бросайте картинки на бесконечный холст, раскладывайте их и держите перед глазами, пока работаете.

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Скачать**](#download) · [Возможности](#features) · [Управление](#getting-around) · [Планы](#roadmap) · [Сборка](#build-from-source) · [Документация](#documentation)

<sub><a href="README.md">English</a> · <b>Русский</b> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="Доска MothRef с тремя иллюстрациями на тёмном холсте" width="900">

</div>

> [!WARNING]
> MothRef в стадии **ранней альфы** (`0.1.0-alpha`). Сохранение, восстановление и отмена работают, но долгие проверки надёжности ещё впереди. Храните отдельные копии важных досок и исходных картинок.

---

<a id="why-mothref"></a>

## Зачем MothRef

[BeeRef](https://github.com/rbreu/beeref) был отличной открытой доской референсов, но его развитие остановилось. [PureRef](https://www.pureref.com) быстрый и отполированный, но закрытый. MothRef продолжает дело BeeRef: нативный, открытый и рассчитанный на огромные доски без пожирания оперативной памяти.

В его основе три обещания, именно в таком порядке:

| Обещание | Что это значит на практике |
| --- | --- |
| **Работа не теряется** | Каждое изменение сначала попадает в журнал восстановления и только потом считается выполненным. Сбой стоит не больше последней секунды. Файлы досок никогда не перезаписываются на месте. |
| **Память по бюджету, а не по размеру доски** | У каждого кэша есть лимит, и давно не использованные картинки вытесняются. Вы сами выбираете баланс между скоростью зума и памятью. |
| **Интерфейс никогда не ждёт** | Декодирование, работа с диском и сжатие идут вне главного потока. Простаивающая доска нагружает процессор и видеокарту на 0%. |

Доски сохраняются в файлы `.moth`. Формат [задокументирован](docs/FORMAT.md), хранит исходные байты картинок без перекодирования и не дублирует одинаковые изображения.

<a id="features"></a>

## Возможности

Что уже работает в текущей альфе:

**Холст**
- Холст на GPU через `wgpu` (Vulkan, Metal, DirectX 12) с программным рендером на случай проблем (`--software`)
- Плавный зум к курсору, инерционная прокрутка пальцем, щипок и жесты тачпада
- Вкладки досок и стартовый экран с библиотекой досок
- Миникарта, редактируемый масштаб, размер выделения и расход памяти в строке состояния
- Анимации с настройкой «меньше движения»

**Картинки и заметки**
- Вставка (<kbd>Ctrl</kbd>+<kbd>V</kbd>), перетаскивание или открытие файлов PNG, JPEG, WebP, GIF и BMP
- Текстовые заметки: двойной щелчок по пустому холсту создаёт заметку, двойной щелчок по заметке открывает редактирование
- Поворот, отражение, кадрирование, масштаб, прозрачность и оттенки серого — всё с отменой

**Раскладка**
- Перемещение, копирование с Alt, сдвиг стрелками
- Выравнивание, распределение, стопки, порядок слоёв и плотная упаковка картинок разного размера
- Показать всю доску или выделение одним нажатием <kbd>Space</kbd>

**Совместимость**
- Открывает доски **BeeRef** `.bee`: картинки, текстовые заметки, поворот, отражение и масштаб
- Открывает доски **PureRef** `.pur` (1.10 и 2.x): встроенные картинки и простые заметки; о неподдерживаемых элементах MothRef сообщает, а не теряет их молча

**Экспорт**
- Доски, выделение или кадрированные области в PNG, SVG и PDF
- Исходные картинки ровно в том виде, в каком их добавили, по одной или архивом ZIP
- Пипетка, копирующая цвет в HEX, RGB или HSV

**Скорость и память**
- Режимы производительности как в PureRef: *В памяти* (по умолчанию), *В памяти, сниженное качество зума*, *Кэш на диске*, *Без кэша*
- Каждая картинка декодируется один раз на диапазон зума, дальше используются закэшированные уровни детализации
- Кэш в RAM ограничен 25% доступной памяти (максимум 4 ГиБ), кэш на диске — 2 ГиБ

**Под себя**
- Любое сочетание клавиш и кнопок мыши можно переназначить, конфликты видны заранее
- Пресеты навигации в стиле BeeRef и PureRef
- Палитра команд (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Интерфейс на английском и русском ([добавить свой язык](docs/LOCALIZATION.md))

<details>
<summary><b>Библиотека досок (скриншот)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="Библиотека досок MothRef при первом запуске" width="900">
</details>

<a id="download"></a>

## Скачать

Свежая сборка лежит в **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| Платформа | Файл | Как запустить |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Распакуйте и запустите `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Распакуйте и запустите `mothref` из Терминала |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Распакуйте и запустите `mothref` из Терминала |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Распакуйте и запустите `./mothref` (Ubuntu 24.04 или новее либо совместимый дистрибутив) |

Сборки не подписаны, а для macOS ещё не нотаризованы, поэтому при первом запуске система может попросить подтверждение. К каждому релизу прилагаются контрольные суммы SHA-256.

Установщики и пакетные менеджеры (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR и другие) [запланированы](ROADMAP.md#distribution) начиная с версии 0.3.
Также запланированы приложение для **Android** и **веб-версия**; ядро уже пишется так, чтобы его можно было перенести на обе платформы.

<a id="getting-around"></a>

## Управление

| Действие | Как |
| --- | --- |
| Добавить картинки | Перетащите файлы, <kbd>Ctrl</kbd>+<kbd>V</kbd> или **Вставить изображение** |
| Перемещать холст | Средняя кнопка мыши, <kbd>Space</kbd>+перетаскивание или левая кнопка с инструментом «Рука» (<kbd>H</kbd>) |
| Масштаб | Колесо мыши, щипок или кнопки <kbd>+</kbd> / <kbd>−</kbd> |
| Показать всё | <kbd>Home</kbd> или нажатие <kbd>Space</kbd> без выделения |
| Показать выделение | Нажатие <kbd>Space</kbd> или двойной щелчок по выделенной картинке (повторный — вернуться) |
| Копировать при перетаскивании | <kbd>Alt</kbd>+перетаскивание |
| Повернуть / отразить | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Палитра команд | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Настроить сочетания | <kbd>/</kbd> |

На macOS вместо <kbd>Ctrl</kbd> используйте <kbd>Cmd</kbd>. Полный список и сравнение с PureRef — в [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (на английском).

<details>
<summary><b>Известные ограничения</b></summary>

- Перетаскивание файлов в нативном Wayland пока недоступно (ограничение winit 0.30). Используйте X11/XWayland, передавайте картинки в командной строке или вставляйте их.
- Доска вмещает до 10 000 элементов; превью показывает до 2048 пикселей детализации на картинку. Зум в полном разрешении по тайлам — в планах.
- Ссылки, форматированные заметки, рисунки и часть кадрирований из PureRef пока не импортируются.

</details>

<a id="roadmap"></a>

## Планы

| Релиз | Этап | Обещание | Статус |
| --- | --- | --- | --- |
| **0.1** пре-альфа | M1 | Не теряет данные | альфа-сборки вышли |
| **0.3** публичная альфа | M2, M2.5 | Заменяет BeeRef, открывает `.bee` и `.pur`, зумит быстрее PureRef | в работе |
| **0.7** бета | M3, M4 | Режим поверх окон, расширение для браузера, все функции PureRef 2 | запланировано |
| **1.0** | M5–M7 | Библиотека досок со стопками и тегами, элементы в стиле Milanote, все цели по производительности | запланировано |
| **2.0** | M8, M9 | Веб-версия и совместная работа в реальном времени | позже |
| — | — | Приложение для Android | запланировано |

После 1.0: инструменты для художников, режим ревью и презентации, связка с Obsidian/Markdown и инструмент командной строки.
Полный план, чек-листы функций и цели по производительности — в [ROADMAP.md](ROADMAP.md) (на английском).

<a id="build-from-source"></a>

## Сборка из исходников

Нужен [Rust](https://rustup.rs) версии 1.95 или новее. В Linux также установите заголовки X11/Wayland (`libxkbcommon-dev` и `libwayland-dev` в Debian/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Всегда используйте `--release`: декодеры картинок рассчитаны на оптимизации, отладочные сборки заметно медленнее.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # открыть картинки при запуске
cargo run -p mothref --release -- board.moth                # открыть доску
cargo run -p mothref --release -- --software                # принудительно рендерить на процессоре
```

<details>
<summary><b>Проверки, бенчмарки и фаззинг</b></summary>

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

`xtask` собирает оптимизированные сборки, чтобы замеры не включали накладные расходы отладки. `bench --quick --enforce` на эталонном железе проверяет бюджет кадра 8 мс (p99), а `bench --quick --software` проверяет программный адаптер.

Для фаззинга декодеров установите `cargo-fuzz` и nightly-тулчейн:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

Есть также цели фаззинга `moth_reader`, `bee_reader` и `pur_reader`.

</details>

<details>
<summary><b>Структура проекта</b></summary>

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

## Документация

- [Архитектура](docs/ARCHITECTURE.md), [архитектурные решения](docs/decisions/)
- [Формат файлов `.moth`](docs/FORMAT.md)
- [Управление и сравнение сочетаний с PureRef](docs/SHORTCUTS.md)
- [Заметки об импорте PureRef](docs/PUR-IMPORT.md)
- [Скорость загрузки](docs/LOAD-PERFORMANCE.md)
- [Как добавить язык](docs/LOCALIZATION.md)
- [Процесс выпуска релизов](docs/RELEASING.md), [список изменений](CHANGELOG.md)

Документация пока на английском.

<a id="contributing"></a>

## Участие

Сообщения об ошибках и идеи ждём в [Issues](https://github.com/hlophlopgaming/mothRef/issues). Прежде чем присылать код, прочитайте [AGENTS.md](AGENTS.md): там правила архитектуры, правила сохранности данных и список проверок. Он одинаково относится и к людям, и к ИИ-агентам.

<a id="license-and-credits"></a>

## Лицензия и благодарности

MothRef — свободная программа под лицензией **[GNU GPL v3.0 или новее](LICENSE)**. Сторонние шрифты и код перечислены в [THIRD_PARTY.md](THIRD_PARTY.md).

Импорт PureRef основан на форке [`purcore`](https://github.com/achrllrogia45/LazyRef) из LazyRef (MIT). Названия «BeeRef», «PureRef» и «Milanote» упоминаются только для описания совместимости; MothRef не связан с ними.

Иллюстрации на скриншоте — работы [David Revoy](https://www.davidrevoy.com): [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) и [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), лицензия [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Иллюстрации уменьшены для показа.

<div align="center">
<br>
<sub>Сделано на Rust и с большим количеством моли.</sub>
</div>
