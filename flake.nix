{
  description = "Claude Code Log Viewer - TUI for viewing Claude Code JSONL logs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-25.11";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ self, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];

      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      perSystem =
        {
          config,
          self',
          inputs',
          system,
          pkgs,
          lib,
          ...
        }:
        let
          # Apply rust-overlay to get rust-bin attribute
          overlays = [
            inputs.rust-overlay.overlays.default
            (final: prev: {
              # Rust toolchain with required extensions and musl targets
              myRustToolchain = final.rust-bin.stable.latest.default.override {
                extensions = [
                  "rust-src"
                  "rust-analyzer"
                  "llvm-tools-preview"
                ];
                targets = [
                  "x86_64-unknown-linux-musl"
                  "aarch64-unknown-linux-musl"
                ];
              };

              # Create rustPlatform using our custom toolchain
              myRustPlatform = final.makeRustPlatform {
                cargo = final.myRustToolchain;
                rustc = final.myRustToolchain;
              };

              libsecret = if final.stdenv.hostPlatform.isMusl
                # test 21 hangs for some reason
                then prev.libsecret.overrideAttrs { doCheck = false; }
                else prev.libsecret;
            })
          ];
          pkgs' = import inputs.nixpkgs {
            inherit system overlays;
          };

          rustToolchain = pkgs'.myRustToolChain;
          rustPlatform = pkgs'.myRustPlatform;

          # Determine static build target based on platform
          isLinux = pkgs'.stdenv.isLinux;
          staticTarget =
            if pkgs'.stdenv.hostPlatform.isx86_64 then
              "x86_64-unknown-linux-musl"
            else if pkgs'.stdenv.hostPlatform.isAarch64 then
              "aarch64-unknown-linux-musl"
            else
              throw "Unsupported platform for static builds: ${system}";

          # Common package metadata
          packageMeta = with lib; {
            description = "TUI application for viewing Claude Code JSONL session logs";
            homepage = "https://github.com/albertov/cclv";
            license = licenses.mit;
            maintainers = [ ];
            mainProgram = "cclv";
          };

          # Cargo hash for vendored dependencies
          # Run: nix build 2>&1 | grep "got:" to get the actual hash
          cargoHash = "sha256-CTb6cx+bJ6PUt3XSAZ0iCKdtuyGnOBwIR9T7VQtKOkE=";

        in
        {
          # Configure treefmt for code formatting
          treefmt = import ./nix/treefmt.nix { inherit pkgs rustToolchain; };

          # Default package (dynamic linking)
          packages.default = rustPlatform.buildRustPackage {
            pname = "cclv";
            version = "0.1.0";
            src = ./.;
            inherit cargoHash;
            doCheck = true;
            dontStrip = false;
            meta = packageMeta;
          };

          # Static package for Linux (fully static, no glibc dependency)
          packages.static = lib.mkIf isLinux (
            pkgs'.pkgsCross.musl64.myRustPlatform.buildRustPackage {
              pname = "cclv";
              version = "0.1.0";
              src = ./.;
              inherit cargoHash;
              doCheck = false; #FIXME

              CARGO_BUILD_TARGET = staticTarget;
              RUSTFLAGS = "-C target-feature=+crt-static";

              meta = packageMeta // {
                description = "TUI application for viewing Claude Code JSONL session logs (static build)";
                platforms = lib.platforms.linux;
              };
            }
          );

          # Development shell
          devShells.default = pkgs'.mkShell (
            import ./nix/devshell.nix {
              pkgs = pkgs';
              inherit rustToolchain self';
            }
          );
        };
    };
}
