# Builds the praxis-website binary. Templates and assets are compiled into
# it, so the package needs no other files at runtime.

{
  lib,
  makeRustPlatform,
  rust-bin,
}:

let
  cargoToml = lib.importTOML ../Cargo.toml;
  toolchain = rust-bin.stable.latest.minimal;
  rustPlatform = makeRustPlatform {
    cargo = toolchain;
    rustc = toolchain;
  };
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  # Only what the build needs, so that changes to docs or CI don't rebuild.
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.lock
      ../Cargo.toml
      ../askama.toml
      ../assets
      ../build.rs
      ../content
      ../src
      ../templates
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  meta = {
    inherit (cargoToml.package) description homepage;
    license = with lib.licenses; [
      asl20
      mit
    ];
    mainProgram = "praxis-website";
    platforms = lib.platforms.linux;
  };
}
