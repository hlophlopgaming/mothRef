<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="MothRef のロゴ：ピクセルアートの蛾" width="128" height="128">
</picture>

# MothRef

**アーティストのための、速くてクラッシュに強いリファレンスボード。**<br>
無限のキャンバスに画像をドロップして並べ、作業中いつでも目に入るようにしておけます。

[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**ダウンロード**](#download) · [機能](#features) · [操作方法](#getting-around) · [ロードマップ](#roadmap) · [ソースからビルド](#build-from-source) · [ドキュメント](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <b>日本語</b> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="暗いキャンバスに 3 枚のイラストを並べた MothRef のボード" width="900">

</div>

> [!WARNING]
> MothRef は**初期アルファ版**（`0.1.0-alpha`）です。保存・復元・元に戻すは動作しますが、長時間の信頼性検証はまだ終わっていません。大切なボードと元画像は別にコピーを保管してください。

---

<a id="why-mothref"></a>

## MothRef を作る理由

[BeeRef](https://github.com/rbreu/beeref) は素晴らしいオープンソースのリファレンスボードでしたが、開発が止まってしまいました。[PureRef](https://www.pureref.com) は速くて洗練されていますが、クローズドソースです。MothRef は BeeRef の続きを担います。ネイティブでオープン、そしてメモリを食いつぶさずに巨大なボードを扱えるように作られています。

次の 3 つの約束を、この優先順位で守ります。

| 約束 | 実際にはどういうことか |
| --- | --- |
| **作業は決して失われない** | すべての変更は、完了とみなされる前に復元用ジャーナルへ書き込まれます。クラッシュしても失うのは最大で直前の 1 秒です。ボードファイルをその場で上書きすることはありません。 |
| **メモリはボードの大きさではなく予算に従う** | すべてのキャッシュに上限があり、最も長く使われていない画像から追い出されます。ズームの速さとメモリのバランスは自分で選べます。 |
| **インターフェースは決して待たない** | デコード、ディスク処理、圧縮はメインスレッドの外で行われます。操作していないボードの CPU・GPU 使用率は 0% です。 |

ボードの拡張子は `.moth` です。形式は[ドキュメント化](docs/FORMAT.md)されており、画像の元のバイト列を再エンコードせずにそのまま保存し、同じ画像は重複して保存しません。

<a id="features"></a>

## 機能

現在のアルファ版で使える機能：

**キャンバス**
- `wgpu` による GPU キャンバス（Vulkan、Metal、DirectX 12）と、予備のソフトウェアレンダリング（`--software`）
- ポインター位置を中心にしたなめらかなズーム、慣性付きのタッチスクロール、ピンチズーム、トラックパッドのジェスチャー
- ボードのタブと、ボードライブラリのスタート画面
- ミニマップ、編集できるズーム倍率、ステータスバーに選択範囲のサイズとメモリ使用量
- アニメーション（動きを減らす設定あり）

**画像とメモ**
- PNG、JPEG、WebP、GIF、BMP ファイルを貼り付け（<kbd>Ctrl</kbd>+<kbd>V</kbd>）、ドロップ、または開く
- テキストメモ：何もないキャンバスをダブルクリックで作成、メモをダブルクリックで編集
- 回転、反転、トリミング、拡大縮小、不透明度、グレースケール。すべて元に戻せます

**配置**
- 移動、Alt ドラッグで複製、矢印キーで微調整
- 整列、均等配置、重ねる、レイヤー順、サイズの異なる画像の最適な詰め込み
- <kbd>Space</kbd> を軽く押すだけでボード全体または選択範囲を画面に収める

**互換性**
- **BeeRef** の `.bee` ボードを開けます：画像、テキストメモ、回転、反転、拡大率
- **PureRef** の `.pur` ボード（1.10 と 2.x）を開けます：埋め込み画像とプレーンなメモ。未対応の要素は通知され、黙って捨てられることはありません

**書き出し**
- ボード、選択範囲、トリミングした領域を PNG、SVG、PDF に書き出し
- 元画像を追加したときのまま、1 枚ずつまたは ZIP で書き出し
- HEX、RGB、HSV をコピーできるカラーピッカー

**速度とメモリ**
- PureRef と同様のパフォーマンスモード：*In memory*（既定）、*In memory, low zoom quality*、*Cache on disk*、*No cache*
- 各画像はズーム範囲ごとに 1 回だけデコードされ、以降のズームではキャッシュ済みの詳細レベルを再利用します
- RAM キャッシュは使用可能メモリの 25%（最大 4 GiB）、ディスクキャッシュは 2 GiB まで

**自分好みに**
- キーボードとマウスの割り当てはすべて変更でき、競合は事前に確認できます
- BeeRef 風・PureRef 風のナビゲーションプリセット
- コマンドパレット（<kbd>Ctrl</kbd>+<kbd>K</kbd>）
- 英語とロシア語のインターフェース（[あなたの言語を追加](docs/LOCALIZATION.md)）

<details>
<summary><b>ボードライブラリ（スクリーンショット）</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="初回起動時の MothRef ボードライブラリ" width="900">
</details>

<a id="download"></a>

## ダウンロード

最新版は **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)** から入手できます。

| プラットフォーム | ファイル | 起動方法 |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | 展開して `mothref.exe` を実行 |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | 展開してターミナルから `mothref` を実行 |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | 展開してターミナルから `mothref` を実行 |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | 展開して `./mothref` を実行（Ubuntu 24.04 以降、または互換ディストリビューション） |

バイナリは署名されておらず、macOS 版はまだ公証されていないため、初回起動時に確認を求められることがあります。各リリースには SHA-256 チェックサムが付属します。

インストーラーとパッケージマネージャー（MSI、`.dmg`、AppImage、Flatpak、Homebrew、AUR など）は 0.3 以降で[予定](ROADMAP.md#distribution)しています。
**Android** アプリと **Web** 版も予定しています。コア部分はすでに両方へ移植できるように書かれています。

<a id="getting-around"></a>

## 操作方法

| やりたいこと | 操作 |
| --- | --- |
| 画像を追加 | ファイルをドロップ、<kbd>Ctrl</kbd>+<kbd>V</kbd>、または **Paste image** |
| パン（表示の移動） | 中ボタンでドラッグ、<kbd>Space</kbd>+ドラッグ、または手のひらツール（<kbd>H</kbd>）で左ドラッグ |
| ズーム | マウスホイール、ピンチ、または <kbd>+</kbd> / <kbd>−</kbd> ボタン |
| 全体を表示 | <kbd>Home</kbd>、または何も選択せずに <kbd>Space</kbd> を軽く押す |
| 選択範囲を表示 | <kbd>Space</kbd> を軽く押す、または選択中の画像をダブルクリック（もう一度で元に戻る） |
| ドラッグしながら複製 | <kbd>Alt</kbd>+ドラッグ |
| 回転 / 反転 | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| コマンドパレット | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| ショートカットの編集 | <kbd>/</kbd> |

macOS では <kbd>Ctrl</kbd> の代わりに <kbd>Cmd</kbd> を使います。全一覧と PureRef との比較は [docs/SHORTCUTS.md](docs/SHORTCUTS.md)（英語）にあります。

<details>
<summary><b>既知の制限</b></summary>

- ネイティブ Wayland ではまだファイルのドロップができません（winit 0.30 の制限）。X11/XWayland を使う、コマンドラインで画像を渡す、または貼り付けてください。
- ボードは最大 10,000 アイテムまで対応し、プレビューは画像 1 枚あたり最大 2048 px の詳細を表示します。タイルによるフル解像度ズームはロードマップにあります。
- PureRef のリンク、リッチテキストのメモ、描画、一部のトリミングはまだ読み込めません。

</details>

<a id="roadmap"></a>

## ロードマップ

| リリース | マイルストーン | 約束 | 状況 |
| --- | --- | --- | --- |
| **0.1** プレアルファ | M1 | データを失わない | アルファ版公開中 |
| **0.3** パブリックアルファ | M2, M2.5 | BeeRef を置き換え、`.bee` と `.pur` を開き、PureRef より速くズーム | 進行中 |
| **0.7** ベータ | M3, M4 | オーバーレイモード、ブラウザー拡張機能、PureRef 2 と同等の機能 | 予定 |
| **1.0** | M5–M7 | スタックとタグに対応したボードライブラリ、Milanote 風の要素、すべての性能目標を達成 | 予定 |
| **2.0** | M8, M9 | Web 版とリアルタイム共同作業 | 将来 |
| — | — | Android アプリ | 予定 |

1.0 以降：アーティスト向けツール、レビュー・プレゼンテーションモード、Obsidian/Markdown 連携、コマンドラインツール。
計画の全体、機能チェックリスト、性能目標は [ROADMAP.md](ROADMAP.md)（英語）にあります。

<a id="build-from-source"></a>

## ソースからビルド

[Rust](https://rustup.rs) 1.95 以降が必要です。Linux では X11/Wayland のヘッダーもインストールしてください（Debian/Ubuntu では `libxkbcommon-dev` と `libwayland-dev`）。

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

必ず `--release` を付けてください。画像デコーダーは最適化に依存しており、デバッグビルドはかなり遅くなります。

```sh
cargo run -p mothref --release -- image.png reference.jpg   # 起動時に画像を開く
cargo run -p mothref --release -- board.moth                # ボードを開く
cargo run -p mothref --release -- --software                # CPU レンダリングを強制
```

<details>
<summary><b>チェック、ベンチマーク、ファジング</b></summary>

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

`xtask` は最適化ビルドを使うため、計測にデバッグのオーバーヘッドは含まれません。基準ハードウェアで `bench --quick --enforce` を実行すると 8 ms のフレーム予算（p99）を強制でき、`bench --quick --software` で CPU アダプターを試せます。

デコーダーのファジングには `cargo-fuzz` と nightly ツールチェーンをインストールします。

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

`moth_reader`、`bee_reader`、`pur_reader` のファジングターゲットもあります。

</details>

<details>
<summary><b>プロジェクト構成</b></summary>

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

## ドキュメント

- [アーキテクチャ](docs/ARCHITECTURE.md), [設計上の決定](docs/decisions/)
- [`.moth` ファイル形式](docs/FORMAT.md)
- [操作方法と PureRef とのショートカット比較](docs/SHORTCUTS.md)
- [PureRef 読み込みに関するメモ](docs/PUR-IMPORT.md)
- [読み込み性能](docs/LOAD-PERFORMANCE.md)
- [言語の追加方法](docs/LOCALIZATION.md)
- [リリース手順](docs/RELEASING.md), [変更履歴](CHANGELOG.md)

ドキュメントは英語です。

<a id="contributing"></a>

## コントリビュート

バグ報告やアイデアは [Issues](https://github.com/hlophlopgaming/mothRef/issues) へどうぞ。コードを送る前に [AGENTS.md](AGENTS.md) を読んでください。アーキテクチャのルール、データ保護のルール、実行すべきチェックが書かれています。人間にも AI コーディングエージェントにも同じように適用されます。

<a id="license-and-credits"></a>

## ライセンスとクレジット

MothRef は **[GNU GPL v3.0 以降](LICENSE)** のもとで提供される自由ソフトウェアです。サードパーティのフォントとコードは [THIRD_PARTY.md](THIRD_PARTY.md) に記載しています。

PureRef の読み込みは、LazyRef の [`purcore`](https://github.com/achrllrogia45/LazyRef) のフォーク（MIT）を利用しています。「BeeRef」「PureRef」「Milanote」の名前は互換性の説明のためにのみ使用しており、MothRef はこれらと提携していません。

スクリーンショットのイラストは [David Revoy](https://www.davidrevoy.com) の作品です：[Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html)、[Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg)、[Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland)（ライセンス：[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)）。表示のため縮小しています。

<div align="center">
<br>
<sub>Rust とたくさんの蛾でできています。</sub>
</div>
