{
  description = "Neon serverless proxy flake";
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
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
  outputs = inputs @ {
    self,
    flake-parts,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        inputs.treefmt-nix.flakeModule
        ./crane.nix
        ./docker.nix
        ./treefmt.nix
      ];
      systems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin"];
      perSystem = {
        system,
        self',
        ...
      }: let
        pkgs = import self.inputs.nixpkgs {
          inherit system;
          overlays = [(import self.inputs.rust-overlay)];
          config = {
            allowUnfree = true;
            allowBroken = true;
          };
        };
        craneLib = self.inputs.crane.mkLib pkgs;
      in {
        _module.args = {
          inherit pkgs craneLib;
        };
        packages.default = self'.packages.feroxyl;
      };
      flake = {};
    };
}
