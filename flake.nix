# Exports the website as Nix package.

{
  description = "Website of the general practice Dipl.-Med. Dirk Schuster";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    # A current Rust toolchain, as nixos-26.05 is older than `rust-version`.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      # The service only targets Linux.
      systems = [
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems =
        function: nixpkgs.lib.genAttrs systems (system: function nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        default = (pkgs.extend rust-overlay.overlays.default).callPackage ./nix/package.nix { };
      });

      checks = forAllSystems (pkgs: {
        package = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      });
    };
}
