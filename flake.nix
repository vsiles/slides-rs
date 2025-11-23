{
  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url  = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, crane, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        craneLib = crane.mkLib pkgs;
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;

          buildInputs = [
            # Add additional build inputs here
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];
        };
        slides-rs = craneLib.buildPackage (
          commonArgs
          // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
            # Additional environment variables or build phases/hooks can be set
            # here *without* rebuilding all dependency crates
            # MY_CUSTOM_VAR = "some value";
          }
        );
        tests = craneLib.cargoNextest (commonArgs // {
          cargoArtifacts = slides-rs.cargoArtifacts;
        });
        fmt-check = craneLib.cargoFmt (commonArgs // {
          cargoArtifacts = slides-rs.cargoArtifacts;
        });
      in
      {
        checks = {
          inherit slides-rs tests fmt-check;
        };

        packages.default = slides-rs;

        apps.default = {
          type = "app";
          program = "${slides-rs}/bin/slides";
          meta.description = "simple Markdown slides viewer inspired by https://github.com/maaslalani/slides";
        };
        devShells.default = with pkgs; mkShell {
          buildInputs = [
            rust-bin.stable.latest.default
            cargo-machete
          ];
        };
      }
    );
}
