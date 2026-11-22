# 0030 — AVIF decoder

## Context

`avif-decode` 3 replaces libaom with the Rust AV1 decoder `rav1d`. The published
`rav1d` 1.1.0 depends on `paste`, which RustSec lists as unmaintained.

## Decision

Use `avif-decode` 3 in MothRef and patch `rav1d` 1.1.0 to use `pastey` in both
Cargo workspaces. The local copy preserves the upstream decoder source. See
[provenance](../../third_party/rav1d/MOTHREF.md).

## Consequences

Source builds on x86 need NASM. On ARM, `avif-decode` uses `rav1d` without
assembly. The existing AVIF dimension checks stay ahead of decoding, and native
packages carry the decoder's license. CI covers the Android core build and
desktop image corpus.
