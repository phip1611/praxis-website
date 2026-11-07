# Exports the website as Nix package and as NixOS module with a hardened
# systemd service. See docs/deployment.md.

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
      # The service only runs on Linux, as the module is systemd-specific.
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

      nixosModules = rec {
        default = praxis-website;
        # Defaults to the package of this flake, as the Rust of the system's
        # nixpkgs may be too old.
        praxis-website =
          { lib, pkgs, ... }:
          {
            imports = [ ./nix/module.nix ];
            services.praxis-website.package =
              lib.mkDefault
                self.packages.${pkgs.stdenv.hostPlatform.system}.default;
          };
      };

      checks = forAllSystems (pkgs: {
        package = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            # The Rust toolchain comes from rustup, so that the MSRV and
            # nightly are available as well.
            rustup
            # Rebuilds and restarts the dev server on changes (bacon.toml).
            bacon
            # The spell checker of the CI.
            typos
            # The formatter of the Nix files (`treefmt`), also via `nix fmt`.
            nixfmt-tree
          ];
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-tree);
    };
}
