{
  description = "A Rust-based TUI player for the 'Bad Apple' animation.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell {
          buildInputs = [
            # Rust toolchain
            (rust-bin.stable.latest.default.withComponents (with rust-bin; [
              "cargo"
              "clippy"
              "rustc"
              "rustfmt"
            ]))

            # System dependencies
            alsa-lib
            alsa-lib.dev
            pkg-config
            openssl
            udev
          ];

          # Environment variables for Rust
          RUST_SRC_PATH = rust-bin.stable.latest.default.rust-src;
        };
      }
    );
}