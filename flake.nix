{
  description = "wgpu-template";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        #toolchain = pkgs.rust-bin.nightly."2023-04-15".default;
        toolchain = pkgs.rust-bin.nightly.latest.default;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };
      in
      {
        # `nix develop`
        devShells.default = pkgs.mkShell rec {
          buildInputs = with pkgs; [
            toolchain
            pkgconfig

            xorg.libX11
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi

            wayland

            libxkbcommon

            vulkan-loader

            renderdoc
          ];

          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}";
        };

        # `nix build`
        packages.default = rustPlatform.buildRustPackage {
          name = "wgpu-template";

          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl
            toolchain
            rustPlatform.cargoSetupHook
          ];
          buildInputs = with pkgs; [
            openssl
          ];

          RUST_BACKTRACE = 1;
        };
      }
    );
}
