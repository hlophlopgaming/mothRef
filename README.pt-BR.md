<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/mothref-logo-dark.png">
  <img src="docs/assets/mothref-logo-light.png" alt="Logo do MothRef: uma mariposa em pixel art" width="128" height="128">
</picture>

# MothRef

**Um quadro de referências rápido e à prova de falhas para artistas.**<br>
Solte imagens em uma tela infinita, organize-as e mantenha-as à vista enquanto trabalha.

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/mothref/)
[![Latest release](https://img.shields.io/github/v/release/hlophlopgaming/mothRef?include_prereleases&label=release&color=a6c47a)](https://github.com/hlophlopgaming/mothRef/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/hlophlopgaming/mothRef/ci.yml?branch=main&label=CI)](https://github.com/hlophlopgaming/mothRef/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/pure-Rust-orange?logo=rust)](https://www.rust-lang.org)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-lightgrey)
![Planned](https://img.shields.io/badge/planned-Android%20%C2%B7%20Web-lightgrey)

[**Baixar**](#download) · [Recursos](#features) · [Controles](#getting-around) · [Roadmap](#roadmap) · [Compilar](#build-from-source) · [Documentação](#documentation)

<sub><a href="README.md">English</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <b>Português (BR)</b> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pl.md">Polski</a> · <a href="README.sv.md">Svenska</a> · <a href="README.fi.md">Suomi</a> · <a href="README.el.md">Ελληνικά</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>

<br>

<img src="docs/screenshots/board.png" alt="Um quadro do MothRef com três ilustrações sobre uma tela escura" width="900">

</div>

> [!WARNING]
> O MothRef está em **alfa inicial** (`0.1.0-alpha`). Salvar, recuperar e desfazer funcionam, mas os testes longos de confiabilidade ainda estão pendentes. Guarde cópias separadas dos quadros importantes e das imagens originais.

---

<a id="why-mothref"></a>

## Por que o MothRef

O [BeeRef](https://github.com/rbreu/beeref) era um ótimo quadro de referências de código aberto, mas parou de evoluir. O [PureRef](https://www.pureref.com) é rápido e caprichado, mas fechado. O MothRef continua de onde o BeeRef parou: nativo, aberto e feito para quadros enormes sem devorar sua RAM.

Ele se apoia em três promessas, nesta ordem:

| Promessa | O que significa na prática |
| --- | --- |
| **Você nunca perde seu trabalho** | Cada alteração vai para um diário de recuperação antes de ser considerada concluída. Um travamento custa no máximo o último segundo. Os arquivos de quadro nunca são sobrescritos no lugar. |
| **A memória segue um orçamento, não o tamanho do quadro** | Cada cache tem um limite e descarta as imagens usadas há mais tempo. Você escolhe o equilíbrio entre velocidade de zoom e memória. |
| **A interface nunca espera** | Decodificação, disco e compressão rodam fora da thread principal. Um quadro parado usa 0% de CPU e GPU. |

Os quadros usam a extensão `.moth`. O formato é [documentado](docs/FORMAT.md), guarda os bytes originais das imagens sem recodificação e não duplica imagens idênticas.

<a id="features"></a>

## Recursos

O que já funciona no alfa atual:

**Tela**
- Tela na GPU com `wgpu` (Vulkan, Metal, DirectX 12) e renderização por software como alternativa (`--software`)
- Zoom suave em direção ao cursor, rolagem por toque com inércia, pinça e gestos de trackpad
- Abas de quadros e uma tela inicial com a biblioteca de quadros
- Minimapa, nível de zoom editável, tamanho da seleção e uso de memória na barra de status
- Animações com opção de reduzir movimento

**Imagens e notas**
- Cole (<kbd>Ctrl</kbd>+<kbd>V</kbd>), arraste ou abra arquivos PNG, JPEG, WebP, GIF e BMP
- Notas de texto: clique duplo na tela vazia para criar, clique duplo em uma nota para editar
- Girar, espelhar, recortar, redimensionar, opacidade e tons de cinza, tudo com desfazer

**Organização**
- Mover, duplicar arrastando com Alt, ajustes finos com as setas
- Alinhar, distribuir, empilhar, ordem das camadas e empacotamento ideal de imagens de tamanhos diferentes
- Enquadrar o quadro inteiro ou a seleção com um toque em <kbd>Space</kbd>

**Compatibilidade**
- Abre quadros **BeeRef** `.bee`: imagens, notas de texto, rotação, espelhamento e escala
- Abre quadros **PureRef** `.pur` (1.10 e 2.x): imagens incorporadas e notas simples; itens não suportados são informados, nunca descartados em silêncio

**Exportação**
- Quadros, seleções ou regiões recortadas em PNG, SVG e PDF
- Imagens originais exatamente como foram adicionadas, uma a uma ou em ZIP
- Conta-gotas que copia HEX, RGB ou HSV

**Velocidade e memória**
- Modos de desempenho no estilo do PureRef: *In memory* (padrão), *In memory, low zoom quality*, *Cache on disk*, *No cache*
- Cada imagem é decodificada uma vez por faixa de zoom; os passos seguintes reutilizam os níveis de detalhe em cache
- Cache em RAM limitado a 25% da memória disponível (máx. 4 GiB), cache em disco a 2 GiB

**Do seu jeito**
- Todos os atalhos de teclado e mouse são editáveis, com prévia de conflitos
- Predefinições de navegação no estilo do BeeRef e do PureRef
- Paleta de comandos (<kbd>Ctrl</kbd>+<kbd>K</kbd>)
- Interface em inglês e russo ([adicione seu idioma](docs/LOCALIZATION.md))

<details>
<summary><b>Biblioteca de quadros (captura de tela)</b></summary>
<br>
<img src="docs/screenshots/library.png" alt="Biblioteca de quadros do MothRef na primeira execução" width="900">
</details>

<a id="download"></a>

## Baixar

Baixe a versão mais recente em **[GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases)**.

| Plataforma | Arquivo | Como executar |
| --- | --- | --- |
| Windows x86_64 | `mothref-windows-x86_64.zip` | Extraia e execute `mothref.exe` |
| macOS, Apple Silicon | `mothref-macos-arm64.tar.gz` | Extraia e execute `mothref` pelo Terminal |
| macOS, Intel | `mothref-macos-x86_64.tar.gz` | Extraia e execute `mothref` pelo Terminal |
| Linux x86_64 | `mothref-linux-x86_64.tar.gz` | Extraia e execute `./mothref` (Ubuntu 24.04 ou mais recente, ou compatível) |

Os binários não são assinados e as versões para macOS ainda não são notarizadas, então o sistema pode pedir confirmação na primeira execução. Cada versão vem com somas de verificação SHA-256.

Instaladores e gerenciadores de pacotes (MSI, `.dmg`, AppImage, Flatpak, Homebrew, AUR e outros) estão [planejados](ROADMAP.md#distribution) a partir da 0.3.
Um app para **Android** e uma versão **web** também estão planejados; o núcleo já é escrito para ser portável para ambos.

<a id="getting-around"></a>

## Controles

| Para fazer isto | Use |
| --- | --- |
| Adicionar imagens | Arraste arquivos, <kbd>Ctrl</kbd>+<kbd>V</kbd> ou **Paste image** |
| Mover a tela | Arrastar com o botão do meio, <kbd>Space</kbd>+arrastar ou arrastar com o botão esquerdo usando a mão (<kbd>H</kbd>) |
| Zoom | Roda do mouse, pinça ou os botões <kbd>+</kbd> / <kbd>−</kbd> |
| Enquadrar tudo | <kbd>Home</kbd>, ou toque em <kbd>Space</kbd> sem nada selecionado |
| Enquadrar a seleção | Toque em <kbd>Space</kbd> ou dê clique duplo em uma imagem selecionada (de novo para voltar) |
| Duplicar ao arrastar | <kbd>Alt</kbd>+arrastar |
| Girar / espelhar | <kbd>R</kbd> / <kbd>Shift</kbd>+<kbd>H</kbd>, <kbd>Shift</kbd>+<kbd>V</kbd> |
| Paleta de comandos | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Editar atalhos | <kbd>/</kbd> |

No macOS, use <kbd>Cmd</kbd> em vez de <kbd>Ctrl</kbd>. A lista completa e uma comparação com o PureRef estão em [docs/SHORTCUTS.md](docs/SHORTCUTS.md) (em inglês).

<details>
<summary><b>Limitações conhecidas</b></summary>

- Arrastar arquivos no Wayland nativo ainda não funciona (limitação do winit 0.30). Use X11/XWayland, passe as imagens pela linha de comando ou cole-as.
- Os quadros suportam até 10.000 itens; as prévias mostram até 2048 px de detalhe por imagem. Zoom em resolução total por blocos está no roadmap.
- Links, notas formatadas, desenhos e alguns recortes do PureRef ainda não são importados.

</details>

<a id="roadmap"></a>

## Roadmap

| Versão | Marco | Promessa | Status |
| --- | --- | --- | --- |
| **0.1** pré-alfa | M1 | Não perde dados | versões alfa publicadas |
| **0.3** alfa público | M2, M2.5 | Substitui o BeeRef, abre `.bee` e `.pur`, faz zoom mais rápido que o PureRef | em andamento |
| **0.7** beta | M3, M4 | Modo sobreposto, extensão de navegador, paridade de recursos com o PureRef 2 | planejado |
| **1.0** | M5–M7 | Biblioteca de quadros com pilhas e tags, elementos no estilo do Milanote, todas as metas de desempenho | planejado |
| **2.0** | M8, M9 | Versão web e colaboração em tempo real | mais tarde |
| — | — | App para Android | planejado |

Depois da 1.0: ferramentas para artistas, modo de revisão e apresentação, integração com Obsidian/Markdown e uma ferramenta de linha de comando.
O plano completo, as listas de recursos e as metas de desempenho estão em [ROADMAP.md](ROADMAP.md) (em inglês).

<a id="build-from-source"></a>

## Compilar a partir do código-fonte

Você precisa do [Rust](https://rustup.rs) 1.95 ou mais recente. No Linux, instale também os cabeçalhos do X11/Wayland (`libxkbcommon-dev`, `libdbus-1-dev` e `libwayland-dev` no Debian/Ubuntu).

```sh
git clone https://github.com/hlophlopgaming/mothRef.git
cd mothRef
cargo run -p mothref --release
```

Use sempre `--release`: os decodificadores de imagem dependem das otimizações e as builds de depuração são bem mais lentas.

```sh
cargo run -p mothref --release -- image.png reference.jpg   # abrir imagens ao iniciar
cargo run -p mothref --release -- board.moth                # abrir um quadro
cargo run -p mothref --release -- --software                # forçar renderização pela CPU
```

<details>
<summary><b>Verificações, benchmarks e fuzzing</b></summary>

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

O `xtask` usa builds otimizadas para que as medições não incluam a sobrecarga de depuração. Use `bench --quick --enforce` no hardware de referência para exigir o orçamento de 8 ms por quadro (p99), e `bench --quick --software` para testar um adaptador por CPU.

Para fazer fuzzing dos decodificadores, instale o `cargo-fuzz` e uma toolchain nightly:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

Também há alvos de fuzzing para `moth_reader`, `bee_reader` e `pur_reader`.

</details>

<details>
<summary><b>Estrutura do projeto</b></summary>

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

## Documentação

- [Arquitetura](docs/ARCHITECTURE.md), [decisões de design](docs/decisions/)
- [O formato de arquivo `.moth`](docs/FORMAT.md)
- [Controles e comparação de atalhos com o PureRef](docs/SHORTCUTS.md)
- [Notas sobre a importação do PureRef](docs/PUR-IMPORT.md)
- [Desempenho de carregamento](docs/LOAD-PERFORMANCE.md)
- [Como adicionar um idioma](docs/LOCALIZATION.md)
- [Processo de lançamento](docs/RELEASING.md), [registro de alterações](CHANGELOG.md)

A documentação está em inglês.

<a id="contributing"></a>

## Como contribuir

Relatos de bugs e ideias são bem-vindos em [Issues](https://github.com/hlophlopgaming/mothRef/issues). Antes de enviar código, leia o [AGENTS.md](AGENTS.md): ele traz as regras de arquitetura, as regras de segurança dos dados e quais verificações rodar. Vale tanto para pessoas quanto para agentes de IA.

<a id="license-and-credits"></a>

## Licença e créditos

O MothRef é software livre sob a **[GNU GPL v3.0 ou posterior](LICENSE)**. Fontes e código de terceiros estão listados em [THIRD_PARTY.md](THIRD_PARTY.md).

A importação do PureRef usa um fork do [`purcore`](https://github.com/achrllrogia45/LazyRef) do LazyRef (MIT). "BeeRef", "PureRef" e "Milanote" são citados apenas para descrever compatibilidade; o MothRef não tem vínculo com eles.

As ilustrações da captura são de [David Revoy](https://www.davidrevoy.com): [Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html), [Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg) e [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland), sob a licença [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). As ilustrações foram reduzidas para exibição.

<div align="center">
<br>
<sub>Feito com Rust e muitas mariposas.</sub>
</div>
