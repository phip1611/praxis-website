# Developer Guide

How the service works and how to change it.

## Local Development

`nix develop` (or direnv with `use flake`) provides rustup, [bacon], [typos],
and the Nix formatter.

| Command           | Purpose                                                   |
|-------------------|-----------------------------------------------------------|
| `cargo dev`       | Dev server on <http://localhost:8080>                     |
| `bacon`           | Same, rebuilds on changes, and the browser reloads itself |
| `cargo test`      | Unit tests, including HTTP tests of the whole router      |
| `nix flake check` | Builds the Nix package and runs the tests                 |
| `nix fmt`         | Formats the Nix files                                     |

The dev mode needs no configuration.

CI runs rustfmt, clippy, rustdoc, the tests, typos, and `nix flake check`. The
lints in `Cargo.toml` require docs on every item, including private ones.
Items get the lowest visibility that works: private, `pub(super)`, or
`pub(crate)`. Plain `pub` is rejected. Dependencies disable their default
features and list the ones the code needs.

## Architecture

Public pages never run templates per request. At startup, `render` produces
every page and the sitemap, and `delivery` compresses them
(Brotli, gzip) into an immutable snapshot. A request only negotiates the
encoding and checks the ETag.

| Module     | Responsibility                                                      |
|------------|---------------------------------------------------------------------|
| `content`  | Practice data, the page table with paths and metadata, and the news |
| `render`   | askama templates, page frame, JSON-LD, sitemap                      |
| `delivery` | Embedded assets with hashed URLs, compression, the snapshot         |
| `web`      | Router, public and legacy routes, middleware                        |
| `security` | All security code: headers, sanitizing, random tokens               |

## Common Changes

**Opening hours, phone numbers, or address:** change `content/site.toml`.
The page descriptions in that file repeat some of these facts;
tests fail until they match. The business card
(`assets/images/Visitenkarte-*`) is an image with the same facts and has to be
recreated by hand, including the WebP variants (`cwebp -q 80 -m 6`).

**A new page:** add an entry to `pages` in `content/site.toml`, a
`PageBody` variant in `src/content/pages.rs`, a template in `templates/pages/`, and the matching
`when` arm in `templates/page.html`. Navigation, sitemap, and the route tests
pick it up from `PAGES`.

**An old URL that should redirect:** add it to `src/web/legacy.rs`. A test
checks that every target exists.

## Tests

Tests use the fixtures in `src/test_support/`: `TestApp` runs the whole router
in memory. A typical test only states the interesting steps:

```rust
let app = TestApp::new();
let contact = app.get("/kontakt/").await;
check!(contact.text().contains("Stadtbadstraße 1b"));
```

[bacon]: https://dystroy.org/bacon/
[typos]: https://github.com/crate-ci/typos
