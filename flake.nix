{
  description = "Claude Code Log Viewer - TUI for viewing Claude Code JSONL logs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-25.05";

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

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ self, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];

      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      perSystem = { config, self', inputs', system, pkgs, lib, ... }:
        let
          # Apply rust-overlay to get rust-bin attribute
          overlays = [ inputs.rust-overlay.overlays.default ];
          pkgs' = import inputs.nixpkgs {
            inherit system overlays;
          };

          # Rust toolchain with required extensions and musl targets
          rustToolchain = pkgs'.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" ];
            targets = [
              "x86_64-unknown-linux-musl"
              "aarch64-unknown-linux-musl"
            ];
          };

          # naersk configured with our toolchain
          naersk' = pkgs'.callPackage inputs.naersk {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          # Determine static build target based on platform
          isLinux = pkgs'.stdenv.isLinux;
          staticTarget =
            if pkgs'.stdenv.hostPlatform.isx86_64 then
              "x86_64-unknown-linux-musl"
            else if pkgs'.stdenv.hostPlatform.isAarch64 then
              "aarch64-unknown-linux-musl"
            else
              throw "Unsupported platform for static builds: ${system}";

        in
        {
          # Configure treefmt for code formatting
          treefmt = {
            projectRootFile = "flake.nix";
            programs = {
              nixpkgs-fmt.enable = true; # Nix formatting
              rustfmt.enable = true; # Rust formatting
              taplo.enable = true; # TOML formatting
            };
          };

          # Default package (dynamic linking)
          packages.default = naersk'.buildPackage {
            src = ./.;
            doCheck = true;

            meta = with lib; {
              description = "TUI application for viewing Claude Code JSONL session logs";
              homepage = "https://github.com/your-org/cclv";
              license = with licenses; [ mit asl20 ];
              maintainers = [ ];
              mainProgram = "cclv";
            };
          };

          # Static package for Linux (fully static, no glibc dependency)
          packages.static = lib.mkIf isLinux (naersk'.buildPackage {
            src = ./.;
            doCheck = true;

            CARGO_BUILD_TARGET = staticTarget;
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";

            # Use static stdenv for musl builds
            nativeBuildInputs = with pkgs'; [
              pkgsStatic.stdenv.cc
            ];

            meta = with lib; {
              description = "TUI application for viewing Claude Code JSONL session logs (static build)";
              homepage = "https://github.com/your-org/cclv";
              license = with licenses; [ mit asl20 ];
              maintainers = [ ];
              mainProgram = "cclv";
              platforms = platforms.linux;
            };
          });

          # Development shell
          devShells.default = pkgs'.mkShell {
            inputsFrom = [ self'.packages.default ];

            packages = with pkgs'; [
              # Rust toolchain with extensions
              rustToolchain

              # Development utilities
              cargo-watch # Auto-rebuild on file changes
              cargo-edit # cargo add/rm/upgrade commands
              cargo-outdated # Check for outdated dependencies

              # Additional helpful tools
              rust-analyzer # LSP server (also in toolchain extensions)
            ];

            # Environment variables for development
            RUST_BACKTRACE = "1";

            shellHook = ''
              echo "cclv - Claude Code Log Viewer"
              echo "Development environment ready"
              echo ""
              echo "Commands:"
              echo "  cargo build          - Build debug binary"
              echo "  cargo build --release - Build release binary"
              echo "  cargo test           - Run tests"
              echo "  cargo clippy         - Lint code"
              echo "  cargo fmt            - Format Rust code"
              echo "  cargo watch -x run   - Auto-rebuild on changes"
              echo ""
              echo "Nix commands:"
              echo "  nix build            - Build dynamic binary"
              echo "  nix build .#static   - Build static binary (Linux)"
              echo "  nix fmt              - Format all code"
              echo ""
            '';
          };
        };
    };
}
