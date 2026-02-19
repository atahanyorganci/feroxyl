{
  perSystem = {
    pkgs,
    craneLib,
    self',
    ...
  }: let
    inherit (pkgs) lib;
    rustToolchainFor = p:
      p.rust-bin.selectLatestNightlyWith (
        toolchain:
          toolchain.default.override {
            extensions = ["rust-src" "rustfmt"];
          }
      );
    rustToolchain = rustToolchainFor pkgs;
    craneLibNightly = craneLib.overrideToolchain rustToolchainFor;
    src = craneLibNightly.cleanCargoSource ./.;
    commonArgs = {
      inherit src;
      strictDeps = true;
      buildInputs = [] ++ lib.optionals pkgs.stdenv.isDarwin [pkgs.libiconv];
    };
    cargoArtifacts = craneLibNightly.buildDepsOnly commonArgs;
    individualCrateArgs =
      commonArgs
      // {
        inherit cargoArtifacts;
        inherit (craneLibNightly.crateNameFromCargoToml {inherit src;}) version;
        doCheck = false;
      };
    feroxyl = craneLibNightly.buildPackage (
      individualCrateArgs
      // {
        pname = "feroxyl";
        src = src;
      }
    );
  in {
    packages.feroxyl = feroxyl;
    devShells.default = craneLibNightly.devShell {
      checks = self'.checks;
      packages = [rustToolchain];
      RUST_SRC_PATH = "${rustToolchain.passthru.availableComponents.rust-src}/lib/rustlib/src/rust/library";
      DYLD_LIBRARY_PATH = "${rustToolchain.passthru.availableComponents.rustc}/lib";
    };
  };
}
