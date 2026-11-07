# CHANGELOG

## Unreleased

- Initial version: Replaces the WordPress website with a small Rust service.
  All content and public URLs stay available.
- NixOS module with a hardened systemd service.
- The access statistics show the device class, browser, and operating system
  of visitors, without storing user agents.
- The stylesheet and the scripts are minified at build time.
- The PNG images are a quarter smaller, without changing their pixels.
