<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="MothRef 로고: 픽셀 아트 나방" width="128" height="128">
</picture>

# MothRef

**아티스트를 위한 빠르고 충돌에도 안전한 레퍼런스 보드.**<br>
무한 캔버스에 이미지를 끌어다 놓고, 배치하고, 작업하는 동안 늘 눈앞에 두세요.

[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**다운로드**](#download) · [기능](#features) · [조작법](#getting-around) · [로드맵](#roadmap) · [소스에서 빌드](#build-from-source) · [문서](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.pt-BR.md">Português (BR)</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <b>한국어</b></sub>

<br>

<img src="docs/screenshots/board.png" alt="어두운 캔버스 위에 일러스트 세 점이 놓인 MothRef 보드" width="900">

</div>

> [!WARNING]
> MothRef는 **초기 알파** 단계(`0.1.0-alpha`)입니다. 저장, 복구, 실행 취소는 동작하지만 장시간 안정성 검증은 아직 진행 중입니다. 중요한 보드와 원본 이미지는 따로 사본을 보관하세요.

---

<a id="why-mothref"></a>

## 왜 MothRef인가

[BeeRef](https://github.com/rbreu/beeref)는 훌륭한 오픈 소스 레퍼런스 보드였지만 개발이 멈췄습니다. [PureRef](https://www.pureref.com)는 빠르고 완성도가 높지만 비공개 소스입니다. MothRef는 BeeRef가 멈춘 곳에서 이어갑니다. 네이티브이고, 공개되어 있으며, RAM을 잡아먹지 않고도 거대한 보드를 다루도록 만들어졌습니다.

다음 세 가지 약속을 이 순서대로 지킵니다.

| 약속 | 실제로 의미하는 것 |
| --- | --- |
| **작업을 절대 잃지 않습니다** | 모든 변경은 완료로 처리되기 전에 복구 저널에 기록됩니다. 충돌이 나도 잃는 것은 최대 마지막 1초입니다. 보드 파일을 제자리에서 덮어쓰는 일은 없습니다. |
| **메모리는 보드 크기가 아니라 예산을 따릅니다** | 모든 캐시에는 한도가 있고, 가장 오래 사용하지 않은 이미지부터 내보냅니다. 확대 속도와 메모리 사이의 균형은 직접 고를 수 있습니다. |
| **인터페이스는 절대 기다리지 않습니다** | 디코딩, 디스크 작업, 압축은 메인 스레드 밖에서 실행됩니다. 가만히 있는 보드는 CPU와 GPU를 0% 사용합니다. |

보드는 `.moth` 확장자를 사용합니다. 형식은 [문서화](docs/FORMAT.md)되어 있으며, 이미지의 원본 바이트를 재인코딩 없이 그대로 저장하고 같은 이미지는 한 번만 저장합니다.

<a id="features"></a>

## 기능

현재 알파 버전에서 이미 동작하는 기능:

**캔버스**
- `wgpu` 기반 GPU 캔버스(Vulkan, Metal, DirectX 12)와 대체용 소프트웨어 렌더링(`--software`)
- 포인터를 중심으로 한 부드러운 확대, 관성이 있는 터치 이동, 핀치 확대, 트랙패드 제스처
- 보드 탭과 보드 라이브러리가 있는 시작 화면
- 미니맵, 직접 입력할 수 있는 확대 배율, 상태 표시줄의 선택 크기와 메모리 사용량
- 움직임 줄이기 설정이 있는 애니메이션

**이미지와 메모**
- PNG, JPEG, WebP, GIF, BMP 파일 붙여넣기(<kbd>Ctrl</kbd>+<kbd>V</kbd>), 끌어다 놓기, 열기
- 텍스트 메모: 빈 캔버스를 더블클릭하면 새로 만들고, 메모를 더블클릭하면 편집합니다
- 회전, 뒤집기, 자르기, 크기 조절, 불투명도, 흑백. 모두 실행 취소할 수 있습니다

**정렬과 배치**
- 이동, Alt 드래그로 복제, 화살표 키로 미세 이동
- 맞춤, 분배, 쌓기, 레이어 순서, 크기가 다른 이미지의 최적 배치
- <kbd>Space</kbd>를 한 번 누르면 보드 전체나 선택 영역을 화면에 맞춤

**호환성**
- **BeeRef** `.bee` 보드 열기: 이미지, 텍스트 메모, 회전, 뒤집기, 크기
- **PureRef** `.pur` 보드(1.10 및 2.x) 열기: 포함된 이미지와 일반 메모. 지원하지 않는 항목은 알려 주며, 몰래 버리지 않습니다

**내보내기**
- 보드, 선택 영역, 잘라낸 영역을 PNG, SVG, PDF로
- 추가했을 때 그대로의 원본 이미지를 하나씩 또는 ZIP으로
- HEX, RGB, HSV를 복사하는 색상 피커

**속도와 메모리**
- PureRef와 같은 성능 모드: *In memory*(기본값), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- 각 이미지는 확대 구간마다 한 번만 디코딩되고, 이후 확대 단계에서는 캐시된 세부 수준을 재사용합니다
- RAM 캐시는 사용 가능한 메모리의 25%(최대 4 GiB), 디스크 캐시는 2 GiB로 제한

**내 방식대로**
- 모든 키보드·마우스 단축키를 바꿀 수 있으며, 충돌을 미리 보여 줍니다
- BeeRef 스타일과 PureRef 스타일의 탐색 프리셋
- 명령 팔레트(<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- 영어와 러시아어 인터페이스([내 언어 추가하기](docs/LOCALIZATION.md))

<details>
<summary><b>보드 라이브러리(스크린샷)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="처음 실행했을 때의 MothRef 보드 라이브러리" width="900">
</details>

<a id="download"></a>

## 다운로드

최신 빌드는 **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**에서 받을 수 있습니다.

| 플랫폼 | 파일 | 실행 방법 |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | 압축을 풀고 `mothref.exe` 실행 |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | 압축을 풀고 터미널에서 `mothref` 실행 |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | 압축을 풀고 터미널에서 `mothref` 실행 |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | 압축을 풀고 `./mothref` 실행(Ubuntu 24.04 이상 또는 호환 배포판) |

바이너리는 서명되지 않았고 macOS 빌드는 아직 공증되지 않았으므로, 처음 실행할 때 시스템이 확인을 요청할 수 있습니다. 모든 릴리스에 SHA-256 체크섬이 함께 제공됩니다.

설치 프로그램과 패키지 관리자(MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR 등)는 0.3부터 [계획](ROADMAP.md#distribution)되어 있습니다.
**Android** 앱과 **웹** 버전도 계획되어 있습니다. 핵심 코드는 지금부터 두 플랫폼으로 옮길 수 있게 작성되고 있습니다.

<a id="getting-around"></a>

## 조작법

| 하고 싶은 일 | 방법 |
| --- | --- |
| 이미지 추가 | 파일 끌어다 놓기, <kbd>Ctrl</kbd>+<kbd>V</kbd>, 또는 **Paste image** |
| 화면 이동 | 가운데 버튼 드래그, <kbd>Space</kbd>+드래그, 또는 손 도구(<kbd>H</kbd>)로 왼쪽 버튼 드래그 |
| 확대/축소 | 마우스 휠, 핀치, 또는 <kbd>+</kbd> / <kbd>−</kbd> 버튼 |
| 전체 보기 | <kbd>Home</kbd>, 또는 아무것도 선택하지 않은 상태에서 <kbd>Space</kbd> |
| 선택 영역 보기 | <kbd>Space</kbd>를 누르거나 선택한 이미지를 더블클릭(다시 하면 돌아감) |
| 드래그하며 복제 | <kbd>Alt</kbd>+드래그 |
| 회전 / 뒤집기 | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| 명령 팔레트 | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| 단축키 편집 | <kbd>/</kbd> |

macOS에서는 <kbd>Ctrl</kbd> 대신 <kbd>Cmd</kbd>를 사용하세요. 전체 목록과 PureRef 비교는 [docs/SHORTCUTS.md](docs/SHORTCUTS.md)(영어)에 있습니다.

<details>
<summary><b>알려진 제한 사항</b></summary>

- 네이티브 Wayland에서는 아직 파일 끌어다 놓기를 쓸 수 없습니다(winit 0.30의 제한). X11/XWayland를 사용하거나, 명령줄로 이미지를 넘기거나, 붙여넣으세요.
- 보드는 최대 10,000개 항목을 지원하며, 미리보기는 이미지당 최대 2048px까지 세부를 보여 줍니다. 타일 기반 원본 해상도 확대는 로드맵에 있습니다.
- PureRef의 링크, 서식 있는 메모, 그림, 일부 자르기는 아직 가져오지 않습니다.

</details>

<a id="roadmap"></a>

## 로드맵

| 릴리스 | 마일스톤 | 약속 | 상태 |
| --- | --- | --- | --- |
| **0.1** 프리알파 | M1 | 데이터를 잃지 않음 | 알파 빌드 공개 |
| **0.3** 공개 알파 | M2, M2.5 | BeeRef 대체, `.bee`와 `.pur` 열기, PureRef보다 빠른 확대 | 진행 중 |
| **0.7** 베타 | M3, M4 | 오버레이 모드, 브라우저 확장 프로그램, PureRef 2와 같은 기능 | 계획됨 |
| **1.0** | M5–M7 | 스택과 태그를 지원하는 보드 라이브러리, Milanote 스타일 요소, 모든 성능 목표 달성 | 계획됨 |
| **2.0** | M8, M9 | 웹 버전과 실시간 협업 | 나중에 |
| — | — | Android 앱 | 계획됨 |

1.0 이후: 아티스트용 도구, 리뷰·프레젠테이션 모드, Obsidian/Markdown 연동, 명령줄 도구.
전체 계획, 기능 체크리스트, 성능 목표는 [ROADMAP.md](ROADMAP.md)(영어)에 있습니다.

<a id="build-from-source"></a>

## 소스에서 빌드

[Rust](https://rustup.rs) 1.95 이상이 필요합니다. Linux에서는 X11/Wayland 헤더도 설치하세요(Debian/Ubuntu에서는 `libxkbcommon-dev`와 `libwayland-dev`).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

항상 `--release`를 사용하세요. 이미지 디코더는 최적화에 의존하며, 디버그 빌드는 훨씬 느립니다.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # 시작할 때 이미지 열기
cargo run -p mothref --release -- board.moth                # 보드 열기
cargo run -p mothref --release -- --software                # CPU 렌더링 강제
```

<details>
<summary><b>검사, 벤치마크, 퍼징</b></summary>

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

`xtask`는 최적화 빌드를 사용하므로 측정값에 디버그 오버헤드가 포함되지 않습니다. 기준 하드웨어에서 `bench --quick --enforce`로 8 ms 프레임 예산(p99)을 강제하고, `bench --quick --software`로 CPU 어댑터를 시험할 수 있습니다.

디코더를 퍼징하려면 `cargo-fuzz`와 nightly 툴체인을 설치하세요:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

`moth_reader`, `bee_reader`, `pur_reader`용 퍼징 대상도 있습니다.

</details>

<details>
<summary><b>프로젝트 구조</b></summary>

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

## 문서

- [아키텍처](docs/ARCHITECTURE.md), [설계 결정](docs/decisions/)
- [`.moth` 파일 형식](docs/FORMAT.md)
- [조작법과 PureRef 단축키 비교](docs/SHORTCUTS.md)
- [PureRef 가져오기 참고 사항](docs/PUR-IMPORT.md)
- [불러오기 성능](docs/LOAD-PERFORMANCE.md)
- [언어 추가하기](docs/LOCALIZATION.md)
- [릴리스 절차](docs/RELEASING.md), [변경 기록](CHANGELOG.md)

문서는 영어로 되어 있습니다.

<a id="contributing"></a>

## 기여하기

버그 제보와 아이디어는 [Issues](https://github.com/hlophlopgaming/mothRef/issues)에 남겨 주세요. 코드를 보내기 전에 [AGENTS.md](AGENTS.md)를 읽어 주세요. 아키텍처 규칙, 데이터 보호 규칙, 실행할 검사가 정리되어 있습니다. 사람과 AI 코딩 에이전트 모두에게 똑같이 적용됩니다.

<a id="license-and-credits"></a>

## 라이선스와 감사의 말

MothRef는 **[GNU GPL v3.0 이상](LICENSE)** 라이선스의 자유 소프트웨어입니다. 서드파티 글꼴과 코드는 [THIRD_PARTY.md](THIRD_PARTY.md)에 정리되어 있습니다.

PureRef 가져오기는 LazyRef의 [`purcore`](https://github.com/achrllrogia45/LazyRef) 포크(MIT)를 기반으로 합니다. 'BeeRef', 'PureRef', 'Milanote'라는 이름은 호환성을 설명하기 위해서만 사용하며, MothRef는 이들과 관련이 없습니다.

스크린샷의 일러스트는 [David Revoy](https://www.davidrevoy.com)의 작품입니다: [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg), [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) 라이선스. 표시를 위해 축소했습니다.

<div align="center">
<br>
<sub>Rust와 수많은 나방으로 만들었습니다.</sub>
</div>
