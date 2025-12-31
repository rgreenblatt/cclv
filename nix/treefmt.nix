# nix/treefmt.nix
{ myRustToolchain, ... }:
{
  projectRootFile = "flake.nix";
  programs = {
    nixfmt.enable = true; # Nix formatting
    deadnix.enable = true; # Nix DCE
    rustfmt.enable = true; # Rust formatting
    rustfmt.package = myRustToolchain;
    taplo.enable = true; # TOML formatting (Cargo.toml)
  };
  /*
    settings.formatter.rustfmt.options = [
      "--edition"
      "2024"
    ];
  */
}
