{
  perSystem = {...}: {
    treefmt = {
      projectRootFile = "flake.nix";
      programs = {
        deadnix.enable = true;
        mdsh.enable = true;
        alejandra.enable = true;
        taplo.enable = true;
      };
      settings.formatter.taplo.options = [
        "--config"
        (builtins.toString ./taplo.toml)
      ];
    };
  };
}
