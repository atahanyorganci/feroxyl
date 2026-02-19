{
  perSystem = {
    pkgs,
    craneLib,
    self',
    ...
  }: let
    inherit (pkgs) lib;
    muslTargets = {
      "aarch64-darwin" = "aarch64-apple-darwin";
      "x86_64-darwin" = "x86_64-apple-darwin";
      "x86_64-linux" = "x86_64-unknown-linux-musl";
      "aarch64-linux" = "aarch64-unknown-linux-musl";
    };
    rustToolchainFor = p:
      p.rust-bin.selectLatestNightlyWith (
        toolchain:
          toolchain.default.override {
            extensions = ["rust-src" "rustfmt"];
            targets = [ muslTargets.${p.stdenv.system} ];
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

    feroxyl =
      if pkgs.stdenv.isLinux
      then
        craneLibNightly.buildPackage (
          individualCrateArgs
          // {
            pname = "feroxyl";
            src = src;
            CARGO_BUILD_TARGET = muslTargets.${pkgs.stdenv.system};
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          }
        )
      else
        craneLibNightly.buildPackage (
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
    };
  };
}
