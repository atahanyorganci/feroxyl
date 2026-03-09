{
  perSystem = {
    pkgs,
    self',
    ...
  }: let
    cargoTomlContents = builtins.readFile ./Cargo.toml;
    cargoToml = builtins.fromTOML cargoTomlContents;
    name = cargoToml.package.name;
    systemToArchitecture = {
      "x86_64-linux" = "amd64";
      "aarch64-linux" = "arm64";
    };
    architecture = systemToArchitecture.${pkgs.stdenv.system};
  in {
    packages = pkgs.lib.mkIf pkgs.stdenv.isLinux {
      feroxyl-image = pkgs.dockerTools.buildLayeredImage {
        inherit name architecture;
        config = {
          Cmd = [
            "${self'.packages.feroxyl}/bin/feroxyl"
            "--address"
            "0.0.0.0"
          ];
        };
      };
    };
  };
}
