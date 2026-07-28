{
  description = "Product-neutral FinTS Rust client development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain ];

          shellHook = ''
            echo "FinTS Rust client dev shell"
            echo "  rustc:     $(rustc --version)"
            echo "  cargo:     $(cargo --version)"
            echo "  clippy:    $(cargo clippy --version)"
            echo "  rustfmt:   $(rustfmt --version)"
            echo "  rust-analyzer: $(rust-analyzer --version)"
          '';
        };
      }
    );
}
