# Interface translations

Translations live in `crates/moth-ui/locales/*.txt` and are embedded at build
time. No translation files need to be installed alongside the application.

To add a language:

1. Copy `en.txt` to a language tag such as `de.txt` or `pt-br.txt` (lowercase).
2. Set `name` to the language's native name, then translate the values after
   ` = `. Keep keys unchanged. Values are literal UTF-8 on one line; punctuation,
   `=`, and backslashes need no escaping. Lines beginning with `#` are comments.
3. Run `cargo test -p moth-ui`. Rebuild the application: the new language appears
   automatically in the language menu and uses the existing saved preference.

English defines the complete key set. Missing keys in other catalogs use English;
empty values, duplicate keys and unknown keys fail the build. Remove untranslated
keys for a partial translation. Adding a UI string requires a documented variant
in `Text` and a matching English key; the compiler checks that they agree.

Locale matching ignores case, accepts OS forms such as `ru_RU.UTF-8`, and tries
an exact language tag before its base language. Unsupported languages retain the
English startup default. Settings continue to store language tags, not indices.

For validation of this change, Windows OS detection needs a manual Windows run;
this session runs on Linux.

Check glyph coverage when adding a new writing system: the current UI embeds
Noto Sans and Hack in `moth_ui::theme`. A catalog does not add fonts or implement
right-to-left layout. Check the translated interface for clipping at small sizes.
