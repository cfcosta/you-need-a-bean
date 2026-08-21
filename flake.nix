{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      pre-commit-hooks,
      rust-overlay,
      treefmt-nix,
      ...
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f (
            let
              pkgs = import nixpkgs {
                inherit system;
                overlays = [ (import rust-overlay) ];
              };

              rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

              rustPlatform = pkgs.makeRustPlatform {
                rustc = rust;
                cargo = rust;
              };

              formatter =
                (treefmt-nix.lib.evalModule pkgs {
                  projectRootFile = "flake.nix";
                  settings.global.excludes = [
                    "*.lock"
                    # Vendored crate: keep byte-comparable to upstream.
                    "vendor/*"
                  ];
                  programs = {
                    nixfmt.enable = true;
                    rustfmt = {
                      enable = true;
                      package = rust;
                    };
                    taplo.enable = true;
                  };
                }).config.build.wrapper;

              preCommitCheck = pre-commit-hooks.lib.${system}.run {
                src = ./.;
                hooks = {
                  deadnix.enable = true;
                  nixfmt-rfc-style.enable = true;
                  treefmt = {
                    enable = true;
                    package = formatter;
                  };
                };
              };

              you-need-a-bean = rustPlatform.buildRustPackage {
                pname = "you-need-a-bean";
                version = "0.1.0";
                src = ./.;
                cargoLock.lockFile = ./Cargo.lock;
                cargoBuildFlags = [
                  "-p"
                  "you-need-a-bean-cli"
                  "--bin"
                  "you-need-a-bean"
                ];
                cargoInstallFlags = [
                  "-p"
                  "you-need-a-bean-cli"
                  "--bin"
                  "you-need-a-bean"
                ];
                doCheck = false;
                meta.mainProgram = "you-need-a-bean";
              };
            in
            {
              inherit
                formatter
                pkgs
                preCommitCheck
                you-need-a-bean
                rust
                ;
            }
          )
        );
    in
    {
      packages = forEachSupportedSystem (
        { you-need-a-bean, ... }:
        {
          default = you-need-a-bean;
          inherit you-need-a-bean;
        }
      );

      formatter = forEachSupportedSystem ({ formatter, ... }: formatter);

      checks = forEachSupportedSystem (
        { preCommitCheck, ... }:
        {
          inherit preCommitCheck;
        }
      );

      devShells = forEachSupportedSystem (
        {
          formatter,
          pkgs,
          rust,
          ...
        }:
        {
          default = pkgs.mkShell {
            name = "you-need-a-bean";
            packages = with pkgs; [
              rust

              bacon
              bun
              cargo-mutants
              cargo-nextest
              formatter
            ];
          };
        }
      );
    };
}
