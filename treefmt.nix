{
  perSystem = {...}: {
    treefmt = {
      projectRootFile = "flake.nix";
      programs = {
        deadnix.enable = true;
        mdsh.enable = true;
        alejandra.enable = true;
      };
    };
  };
}
