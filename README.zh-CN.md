<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="MothRef 标志：一只像素风格的飞蛾" width="128" height="128">
</picture>

# MothRef

**为艺术家打造的快速、不怕崩溃的参考图板。**<br>
把图片拖到无限画布上，自由摆放，工作时随时放在眼前。

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**下载**](#download) · [功能](#features) · [操作](#getting-around) · [路线图](#roadmap) · [从源码构建](#build-from-source) · [文档](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <b>简体中文</b> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="深色画布上摆着三幅插画的 MothRef 图板" width="900">

</div>

> [!WARNING]
> MothRef 目前处于**早期 Alpha** 阶段（`0.1.0-alpha`）。保存、恢复和撤销都已可用，但长时间的可靠性验证尚未完成。请为重要的图板和原始图片另外保留备份。

---

<a id="why-mothref"></a>

## 为什么选择 MothRef

[BeeRef](https://github.com/rbreu/beeref) 曾是一款很棒的开源参考图板，但已停止开发。[PureRef](https://www.pureref.com) 快速精致，但不开源。MothRef 接过 BeeRef 的接力棒：原生、开源，专为超大图板设计，而且不会吃光你的内存。

它围绕三个承诺构建，按优先级排列：

| 承诺 | 实际意味着什么 |
| --- | --- |
| **绝不丢失你的作品** | 每次修改都会先写入恢复日志，才算完成。即使崩溃，最多只损失最后一秒。图板文件从不就地覆盖。 |
| **内存按预算使用，而不是随图板变大** | 每个缓存都有上限，并会淘汰最久未使用的图片。缩放速度和内存占用之间的取舍由你决定。 |
| **界面从不卡住等待** | 解码、磁盘读写和压缩都在主线程之外进行。空闲的图板占用 0% 的 CPU 和 GPU。 |

图板使用 `.moth` 扩展名。该格式[有完整文档](docs/FORMAT.md)，原样保存图片的原始字节（不重新编码），相同的图片只存一份。

<a id="features"></a>

## 功能

当前 Alpha 版本已经可以使用的功能：

**画布**
- 基于 `wgpu` 的 GPU 画布（Vulkan、Metal、DirectX 12），并提供软件渲染作为后备（`--software`）
- 以指针为中心的平滑缩放、带惯性的触摸平移、双指缩放和触控板手势
- 图板标签页，以及带图板库的启动页
- 小地图、可编辑的缩放比例，状态栏实时显示选区尺寸和内存占用
- 动画效果，可设置为减少动态效果

**图片与笔记**
- 粘贴（<kbd>Ctrl</kbd>+<kbd>V</kbd>）、拖入或打开 PNG、JPEG、WebP、GIF 和 BMP 文件
- 文字笔记：双击空白画布新建，双击笔记进行编辑
- 旋转、翻转、裁剪、缩放、不透明度和灰度，全部可以撤销

**排列**
- 移动、按住 Alt 拖动复制、方向键微调
- 对齐、分布、堆叠、图层顺序，以及不同尺寸图片的最优排布
- 轻按 <kbd>Space</kbd> 即可让整个图板或当前选区适应窗口

**兼容性**
- 可打开 **BeeRef** 的 `.bee` 图板：图片、文字笔记、旋转、翻转和缩放
- 可打开 **PureRef** 的 `.pur` 图板（1.10 和 2.x）：内嵌图片和纯文本笔记；不支持的元素会给出提示，绝不悄悄丢弃

**导出**
- 将图板、选区或裁剪区域导出为 PNG、SVG 和 PDF
- 原样导出添加时的原始图片，可单张导出或打包为 ZIP
- 取色器，可复制 HEX、RGB 或 HSV

**速度与内存**
- 类似 PureRef 的性能模式：*In memory*（默认）、*In memory, low zoom quality*、*Cache on disk*、*No cache*
- 每张图片在每个缩放区间只解码一次，之后的缩放复用已缓存的细节层级
- 内存缓存上限为可用内存的 25%（最多 4 GiB），磁盘缓存上限为 2 GiB

**随心定制**
- 所有键盘和鼠标快捷键均可修改，冲突会提前预览
- BeeRef 风格和 PureRef 风格的导航预设
- 命令面板（<kbd>Ctrl</kbd>+<kbd>K</kbd>）
- 界面支持英语和俄语（[添加你的语言](docs/LOCALIZATION.md)）

<details>
<summary><b>图板库（截图）</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="首次启动时的 MothRef 图板库" width="900">
</details>

<a id="download"></a>

## 下载

从 **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)** 下载最新版本。

| 平台 | 文件 | 运行方式 |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | 解压后运行 `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | 解压后在终端中运行 `mothref` |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | 解压后在终端中运行 `mothref` |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | 解压后运行 `./mothref`（需要 Ubuntu 24.04 或更新版本，或兼容的发行版） |

程序未签名，macOS 版本也尚未公证，因此首次运行时系统可能会要求你确认。每个版本都附有 SHA-256 校验和。

安装包和包管理器（MSI、`.dmg`、AppImage、Flatpak、Homebrew、AUR 等）[计划](ROADMAP.md#distribution)从 0.3 版本开始提供。
**Android** 应用和**网页版**也在计划中；核心代码从现在起就按可移植到这两个平台的方式编写。

<a id="getting-around"></a>

## 操作

| 想要 | 操作方式 |
| --- | --- |
| 添加图片 | 拖入文件、<kbd>Ctrl</kbd>+<kbd>V</kbd> 或点击 **Paste image** |
| 平移 | 按住中键拖动、<kbd>Space</kbd>+拖动，或使用抓手工具（<kbd>H</kbd>）时按左键拖动 |
| 缩放 | 鼠标滚轮、双指捏合，或 <kbd>+</kbd> / <kbd>−</kbd> 按钮 |
| 显示全部 | <kbd>Home</kbd>，或在未选中任何内容时轻按 <kbd>Space</kbd> |
| 显示选区 | 轻按 <kbd>Space</kbd>，或双击已选中的图片（再次双击返回） |
| 拖动时复制 | <kbd>Alt</kbd>+拖动 |
| 旋转 / 翻转 | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| 命令面板 | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| 修改快捷键 | <kbd>/</kbd> |

在 macOS 上请用 <kbd>Cmd</kbd> 代替 <kbd>Ctrl</kbd>。完整列表以及与 PureRef 的对比见 [docs/SHORTCUTS.md](docs/SHORTCUTS.md)（英文）。

<details>
<summary><b>已知限制</b></summary>

- 原生 Wayland 下暂不支持拖入文件（winit 0.30 的限制）。请使用 X11/XWayland、通过命令行传入图片，或直接粘贴。
- 图板最多支持 10,000 个元素；预览中每张图片最多显示 2048 像素的细节。基于图块的全分辨率缩放已列入路线图。
- 暂不导入 PureRef 的链接、富文本笔记、绘画和部分裁剪。

</details>

<a id="roadmap"></a>

## 路线图

| 版本 | 里程碑 | 承诺 | 状态 |
| --- | --- | --- | --- |
| **0.1** 预览 Alpha | M1 | 不丢数据 | Alpha 版已发布 |
| **0.3** 公开 Alpha | M2, M2.5 | 取代 BeeRef，可打开 `.bee` 和 `.pur`，缩放比 PureRef 更快 | 进行中 |
| **0.7** Beta | M3, M4 | 悬浮模式、与 PureRef 2 功能看齐 | 计划中 |
| **1.0** | M5–M7 | 支持堆叠和标签的图板库、Milanote 风格元素、达成全部性能目标 | 计划中 |
| **2.0** | M8, M9 | 网页版与实时协作 | 以后 |
| — | — | Android 应用 | 计划中 |

1.0 之后：艺术家工具、审阅与演示模式、Obsidian/Markdown 联动，以及命令行工具。
完整计划、功能清单和性能目标见 [ROADMAP.md](ROADMAP.md)（英文）。

<a id="build-from-source"></a>

## 从源码构建

需要 [Rust](https://rustup.rs) 1.95 或更新版本。在 Linux 上还需安装 X11/Wayland 头文件（Debian/Ubuntu 上为 `libxkbcommon-dev`, `libdbus-1-dev` 和 `libwayland-dev`）。

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

请始终使用 `--release`：图片解码器依赖编译优化，调试构建会慢很多。

```sh
cargo run -p mothref --release -- image.png reference.jpg   # 启动时打开图片
cargo run -p mothref --release -- board.moth                # 打开图板
cargo run -p mothref --release -- --software                # 强制使用 CPU 渲染
```

<details>
<summary><b>检查、基准测试与模糊测试</b></summary>

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

`xtask` 使用优化构建，因此测量结果不含调试开销。在参考硬件上使用 `bench --quick --enforce` 强制执行 8 毫秒帧预算（p99），用 `bench --quick --software` 测试 CPU 适配器。

要对解码器进行模糊测试，请安装 `cargo-fuzz` 和 nightly 工具链：

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

此外还有 `moth_reader`、`bee_reader` 和 `pur_reader` 的模糊测试目标。

</details>

<details>
<summary><b>项目结构</b></summary>

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

## 文档

- [架构](docs/ARCHITECTURE.md), [设计决策](docs/decisions/)
- [`.moth` 文件格式](docs/FORMAT.md)
- [操作方式及与 PureRef 的快捷键对比](docs/SHORTCUTS.md)
- [PureRef 导入说明](docs/PUR-IMPORT.md)
- [加载性能](docs/LOAD-PERFORMANCE.md)
- [添加新语言](docs/LOCALIZATION.md)
- [发布流程](docs/RELEASING.md), [更新日志](CHANGELOG.md)

文档为英文。

<a id="contributing"></a>

## 参与贡献

欢迎在 [Issues](https://github.com/hlophlopgaming/mothRef/issues) 中提交问题和想法。提交代码前请阅读 [AGENTS.md](AGENTS.md)：其中说明了架构规则、数据安全规则以及需要运行的检查。它同样适用于人类和 AI 编程助手。

<a id="license-and-credits"></a>

## 许可与致谢

MothRef 是基于 **[GNU GPL v3.0 或更高版本](LICENSE)** 的自由软件。第三方字体和代码列于 [THIRD_PARTY.md](THIRD_PARTY.md)。

PureRef 导入功能基于 LazyRef 中 [`purcore`](https://github.com/achrllrogia45/LazyRef) 的分支（MIT）。提及“BeeRef”“PureRef”和“Milanote”仅为说明兼容性；MothRef 与它们没有关联。

截图中的插画由 [David Revoy](https://www.davidrevoy.com) 创作：[Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html)、[Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) 和 [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland)，采用 [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) 许可。插画已缩小以便展示。

<div align="center">
<br>
<sub>用 Rust 和许多飞蛾制作。</sub>
</div>
