# Developer Guide

How the service works and how to change it.

## Local Development

`nix develop` (or direnv with `use flake`) provides rustup, [bacon], [typos],
and the Nix formatter.

| Command           | Purpose                                                      |
|-------------------|--------------------------------------------------------------|
| `cargo dev`       | Dev server on <http://localhost:8080>, login `admin`/`admin` |
| `bacon`           | Same, rebuilds on changes, and the browser reloads itself    |
| `cargo test`      | Unit tests, including HTTP tests of the whole router         |
| `nix flake check` | Builds the Nix package and runs the tests                    |
| `nix fmt`         | Formats the Nix files                                        |

The dev mode needs no configuration and keeps its state in `.dev-state/`. It
refuses to listen on other addresses than localhost, as its password is fixed.

CI runs rustfmt, clippy, rustdoc, the tests, typos, and `nix flake check`. The
lints in `Cargo.toml` require docs on every item, including private ones.
Items get the lowest visibility that works: private, `pub(super)`, or
`pub(crate)`. Plain `pub` is rejected. Dependencies disable their default
features and list the ones the code needs.

## Architecture

Public pages never run templates per request. At startup and after each edit,
`render` produces every page and the sitemap, and `delivery`
compresses them (Brotli, gzip) into an immutable snapshot. A request only
negotiates the encoding and checks the ETag.

| Module     | Responsibility                                                                 |
|------------|--------------------------------------------------------------------------------|
| `content`  | Practice data, the page table with paths and metadata, and the editable news   |
| `render`   | askama templates, page frame, JSON-LD, sitemap                                 |
| `delivery` | Embedded assets with hashed URLs, compression, the snapshot                    |
| `web`      | Router, public and legacy routes, login area, middleware                       |
| `security` | All security code: passwords, sessions, CSRF, throttling, headers, sanitizing  |
| `stats`    | Anonymous counters of page views, devices, and browsers, the daily log summary |
| `persist`  | Atomic JSON files in the state directory                                       |

State-changing handlers take `SessionForm` or `SameOriginForm` from
`web::admin::forms`, which apply the origin and CSRF checks.

## Common Changes

**Opening hours, phone numbers, or address:** change `content/site.toml`.
The page descriptions in that file repeat some of these facts;
tests fail until they match. The business card
(`assets/images/Visitenkarte-*`) is an image with the same facts and has to be
recreated by hand, including the WebP variants (`cwebp -q 80 -m 6`).

**A new page:** add an entry to `pages` in `content/site.toml`, a
`PageBody` variant in `src/content/pages.rs`, a template in `templates/pages/`, and the matching
`when` arm in `templates/page.html`. Navigation, sitemap, statistics, and the
route tests pick it up from that file.

**A new editor feature:** the editor (`assets/editor.js`) may only produce
markup that passes the allowlist in `src/security/sanitize.rs`. Formatting
needs a CSS class, not a `style` attribute, as the Content Security Policy
forbids inline styles: add the class to the allowlist, to `assets/style.css`,
and the button to `templates/admin/editor.html`.

**An old URL that should redirect:** add it to `src/web/legacy.rs`. A test
checks that every target exists.

## Tests

Tests use the fixtures in `src/test_support/`: `TestApp` runs the whole router
in memory with a mock clock and an in-memory store, and `LoggedIn` is a
logged-in browser session. A typical test only states the interesting steps:

```rust
let app = TestApp::new();
let session = app.login().await;
session.save_news("<p>Praxis im Urlaub</p>").await;
assert!(app.get("/").await.text().contains("Praxis im Urlaub"));
```

[bacon]: https://dystroy.org/bacon/
[typos]: https://github.com/crate-ci/typos
