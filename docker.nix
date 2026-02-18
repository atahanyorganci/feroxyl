{
  perSystem = {
    pkgs,
    self',
    ...
  }: let
    cargoTomlContents = builtins.readFile ./Cargo.toml;
    cargoToml = builtins.fromTOML cargoTomlContents;
    name = cargoToml.package.name;
  in {
    packages = pkgs.lib.mkIf pkgs.stdenv.isLinux {
      feroxyl-image = pkgs.dockerTools.buildLayeredImage {
        inherit name;
        config = {
          Cmd = [
            "${self'.packages.feroxyl}/bin/feroxyl"
          ];
        };
      };
    };
  };
}
