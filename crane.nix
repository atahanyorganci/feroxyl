{
  perSystem = {
    pkgs,
    craneLib,
    self',
    inputs',
    ...
  }: let
    inherit (pkgs) lib;
    src = craneLib.cleanCargoSource ./.;
    commonArgs = {
      inherit src;
      strictDeps = true;
      buildInputs = [] ++ lib.optionals pkgs.stdenv.isDarwin [pkgs.libiconv];
    };
    cargoArtifacts = craneLib.buildDepsOnly commonArgs;
    individualCrateArgs =
      commonArgs
      // {
        inherit cargoArtifacts;
        inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
        doCheck = false;
      };
    feroxyl = craneLib.buildPackage (
      individualCrateArgs
      // {
        pname = "feroxyl";
        src = src;
      }
    );
  in {
    packages.feroxyl = feroxyl;
    devShells.default = craneLib.devShell {
      # Inherit inputs from checks.
      checks = self'.checks;
      RUST_SRC_PATH = "${inputs'.fenix.packages.stable.rust-src}/lib/rustlib/src/rust/library";
    };
  };
}
