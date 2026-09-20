{ skills }:
{
  config,
  lib,
  pkgs,
  ...
}:

# Skills flake input is the sole SoT. This module only symlinks that store
# path. Do not vendor a second skills/ tree in omapi-overlay.

let
  cfg = config.programs.omapi;
in
{
  options.programs.omapi = {
    enable = lib.mkEnableOption "omapi overlay (stock omp wrap + skills input + Jev gates)";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.omapi or (throw "programs.omapi: pkgs.omapi missing — add the omapi-overlay overlay");
      defaultText = lib.literalExpression "pkgs.omapi";
      description = "omapi wrap package (thin wrap of stock omp).";
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default =
        lib.optional (pkgs ? jev-router) pkgs.jev-router
        ++ lib.optional (pkgs ? cursor-agent-jev) pkgs.cursor-agent-jev;
      defaultText = lib.literalExpression "[ pkgs.jev-router pkgs.cursor-agent-jev ]";
      description = "Additional overlay packages on PATH (router + cursor-agent-jev).";
    };

    skillsSource = lib.mkOption {
      type = lib.types.path;
      default = skills;
      defaultText = lib.literalExpression "inputs.skills";
      description = ''
        Sole skills source of truth: the flake `inputs.skills` store path.
        Home Manager only symlinks this path to ~/.config/omp/agent/skills.
        Do not point this at a second in-repo skills/ copy.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ] ++ cfg.extraPackages;

    xdg.configFile."omp/agent/skills".source = cfg.skillsSource;
  };
}
