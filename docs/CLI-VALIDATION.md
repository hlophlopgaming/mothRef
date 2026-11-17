# Native CLI validation

The native command frontend and `--agent` transport use the same `Operation` enum,
scene command builders and storage worker. No Python is required.

## Test plan

`crates/mothref/tests/cli.rs` invokes the actual built executable using
`CARGO_BIN_EXE_mothref`, isolated `MOTHREF_AGENT_DATA`, repository image/board
fixtures, and no display environment. Coverage:

- Normal subcommands, flags/help/errors and machine JSON; all command help pages.
- Create, multiple local images, automatic placement, explicit coordinates,
  move/scale/rotation/flip, all layout/align/normalize modes, groups, duplication,
  delete, z-order and original geometry.
- Native save/open/check, foreign read-only inspection and conversion; no overwrite.
- Batch arrays/objects/stdin, explicit save, final autosave, dry-run, failure recovery.
- Stateful JSON lines, undo/redo, nonzero errors and no implicit raw-transport save.
- Real PNG pixel content, SVG/PDF structure, byte-identical original exports,
  archive signature/content, native preview bundles and coordinate mapping.
- GUI-action coverage and catalog drift; explicit HTTP image download and rejection
  of non-HTTP URLs. HTTP server coverage needs loopback socket permission.

Parser and mapping unit tests are in the native agent module. The original
Python/Click harness is retired by explicit maintainer request. Historical Python
validation does not count as evidence for the new command parser.

## Results

Validated on Linux on 2026-11-17. All builds/checks/tests below ran inside the
sandbox; no check was rerun outside it. Native integration children have empty
PATH and no DISPLAY/WAYLAND_DISPLAY, proving no executable wrapper or display is
needed for the exercised commands.

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `cargo test -p mothref --test cli --offline -- --skip url_download_uses_explicit_native_http_backend`: **14 passed**, one socket-dependent case filtered out.
- `cargo test -p mothref --bin mothref app::agent:: --offline`: **4 passed** (GUI/CLI startup routing, parser graph/flags, timestamp boundaries and coordinate mapping).
- The HTTP case was attempted separately with `--exact` and **failed before a request**: TcpListener::bind is denied by the sandbox (`PermissionDenied: Operation not permitted`). It remains enabled in desktop CI; no pass is claimed.
- `cargo deny --offline check licenses bans sources`: passed (existing multiple-version warnings remain). Full `cargo deny --offline check` is incomplete because the advisory database lock path is read-only in the sandbox.
- `cargo build -p mothref --release --offline`: passed.
- Ran the release executable from a shell script with native create/add/normalize/scale/arrange/note/info/check/export/preview commands, then a separate two-request JSON-lines session with explicit save and undo/redo. Both completed successfully without executing Python. Local artifacts: `/tmp/mothref-native-commands.3poYk0/references.moth`, `references.png`, and `raw-json.moth`.
- Canonical agent skill passed quick_validate; its content and both catalogs are embedded in the executable. `git diff --check` passed.

Native corruption regression confirmed that appending a checkpoint does not
necessarily remove a physically torn region: check continues to flag the input.
Saving to a fresh native destination produces a clean board and clears the same
session's recovery state; reopening the new file also passes check.

General workspace tests, corpus/fuzz/kill/benchmark suites and Windows/macOS manual
qualification were not run locally. Native CLI integration tests are part of the
existing desktop `cargo test --workspace` CI step; no Python CI setup is needed.
No tier-3 or general stress suite was run.


### Native integration output

```text
   Compiling mothref v0.1.0-alpha.25 (/home/userreallyfun/Документы/GitHub/mothRef/crates/mothref)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.95s
     Running tests/cli.rs (target/debug/deps/cli-9926076888fd9d6d)

running 14 tests
test invalid_image_url_fails_without_touching_board ... ok
test failed_batch_and_raw_error_keep_unsaved_recovery ... ok
test batch_autosaves_and_raw_json_lines_keep_stateful_undo_redo ... ok
test coordinate_limit_errors_leave_saved_bytes_unchanged ... ok
test raw_transport_does_not_autosave_and_native_cli_handles_stdin_batches ... ok
test preview_bundles_report_native_pixels_world_coordinates_and_latest ... ok
test recovered_tail_requires_a_clean_save_as_before_check_passes ... ok
test explicit_zero_coordinates_and_dry_run_are_preserved ... ok
test flags_errors_and_existing_destinations_never_save_changes ... ok
test auto_insert_accounts_for_rotation_groups_and_global_paginated_bounds ... ok
test foreign_boards_support_read_only_inspection_and_native_conversion ... ok
test create_multiple_images_arrange_and_export_without_python ... ok
test native_help_and_catalog_cover_the_compiled_actions ... ok
test all_layout_modes_and_scene_edits_use_native_flags ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.43s


```

### Parser/mapping output

```text
   Compiling mothref v0.1.0-alpha.25 (/home/userreallyfun/Документы/GitHub/mothRef/crates/mothref)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.39s
     Running unittests src/main.rs (target/debug/deps/mothref-066fc1df8b65cdd6)

running 4 tests
test app::agent::preview::tests::timestamps_keep_leap_days_and_utc_boundaries ... ok
test app::agent::cli::tests::startup_keeps_gui_file_arguments_and_recognizes_native_aliases ... ok
test app::agent::cli::tests::command_graph_and_parsers_validate_before_opening_boards ... ok
test app::agent::preview::tests::pixel_mapping_includes_fractional_padding_and_negative_board_origin ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 158 filtered out; finished in 0.00s


```

### Socket restriction

```text
    Blocking waiting for file lock on build directory
    Finished `test` profile [unoptimized + debuginfo] target(s) in 7.47s
     Running tests/cli.rs (target/debug/deps/cli-9926076888fd9d6d)

running 1 test
test url_download_uses_explicit_native_http_backend ... FAILED

failures:

---- url_download_uses_explicit_native_http_backend stdout ----

thread 'url_download_uses_explicit_native_http_backend' (16) panicked at crates/mothref/tests/cli.rs:588:53:
called `Result::unwrap()` on an `Err` value: Os { code: 1, kind: PermissionDenied, message: "Operation not permitted" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    url_download_uses_explicit_native_http_backend

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p mothref --test cli`

```
