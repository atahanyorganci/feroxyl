{
  perSystem = {...}: {
    treefmt = {
      projectRootFile = "flake.nix";
      programs = {
        alejandra.enable = true;
        deadnix.enable = true;
        mdsh.enable = true;
        rustfmt.enable = true;
        taplo.enable = true;
      };
      settings.formatter.taplo.options = [
        "--config"
        (builtins.toString ./taplo.toml)
      ];
    };
  };
}
