{
  description = "git-extend - Git repository management utilities";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = inputs @ {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachSystem [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ] (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {
        inherit system overlays;
      };
      
      rustToolchain = pkgs.rust-bin.stable.latest.default;
      
      git-extend = pkgs.rustPlatform.buildRustPackage {
        pname = "git-extend";
        version = "0.1.0";
        
        src = ./.;
        
        cargoLock = {
          lockFile = ./Cargo.lock;
        };
        
        buildInputs = with pkgs; lib.optionals stdenv.isDarwin [
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];
        
        meta = with pkgs.lib; {
          description = "Git repository management utilities";
          homepage = "https://github.com/uzaaft/git-extend";
          license = licenses.mit;
          maintainers = [];
        };
      };
    in {
      packages = {
        default = git-extend;
        git-extend = git-extend;
      };

      apps = {
        default = {
          type = "app";
          program = "${git-extend}/bin/git-get";
        };
        git-get = {
          type = "app";
          program = "${git-extend}/bin/git-get";
        };
        git-list = {
          type = "app";
          program = "${git-extend}/bin/git-list";
        };
      };

      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rustToolchain
          pkg-config
          openssl
        ] ++ lib.optionals stdenv.isDarwin [
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];
        
        RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
      };
    });
}
