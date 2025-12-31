# nix/devshell.nix
{
  pkgs,
  rustToolchain,
  self',
  ...
}:
{
  inputsFrom = [ self'.packages.default ];

  packages = with pkgs; [
    # Rust toolchain with extensions
    rustToolchain

    # Development utilities
    cargo-watch # Auto-rebuild on file changes
    cargo-edit # cargo add/rm/upgrade commands
    cargo-outdated # Check for outdated dependencies
    cargo-llvm-cov # Code coverage reports

    # Additional helpful tools
    rust-analyzer # LSP server (also in toolchain extensions)
    cargo-flamegraph
    linuxPackages.perf
    asciinema
    asciinema-agg
    jetbrains-mono  # Font for agg gif rendering
  ];

  # Environment variables for development
  RUST_BACKTRACE = "1";

  # Font config for agg (asciinema gif renderer)
  FONTCONFIG_FILE = pkgs.makeFontsConf { fontDirectories = [ pkgs.jetbrains-mono pkgs.noto-fonts-color-emoji ]; };

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
}
