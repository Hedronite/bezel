{ skills }:
{
  config,
  lib,
  pkgs,
  ...
}:

# Skills flake input is the sole SoT. This module only symlinks that store
# path. Do not vendor a second skills/ tree in Bezel.
# `programs.omapi` is a renamed option for `programs.bezel` (one transition).

{
  imports = [
    (lib.mkRenamedOptionModule [ "programs" "omapi" ] [ "programs" "bezel" ])
  ];

  options.programs.bezel = {
    enable = lib.mkEnableOption "Bezel overlay (skills, Jev gates, and an optional omp wrap)";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.bezel or (throw "programs.bezel: pkgs.bezel missing — import the Bezel overlay");
      defaultText = lib.literalExpression "pkgs.bezel";
      description = "Bezel package. Runs upstream omp.";
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default =
        lib.optional (pkgs ? bezel-jev) pkgs.bezel-jev
        ++ lib.optional (pkgs ? cursor-agent-bezel) pkgs.cursor-agent-bezel
        ++ lib.optional (pkgs ? grok-build-bezel) pkgs.grok-build-bezel;
      defaultText = lib.literalExpression "[ pkgs.bezel-jev pkgs.cursor-agent-bezel pkgs.grok-build-bezel ]";
      description = "Additional Bezel packages on PATH (router, Cursor adapter, Grok Build adapter).";
    };

    skillsSource = lib.mkOption {
      type = lib.types.path;
      default = skills;
      defaultText = lib.literalExpression "inputs.skills";
      description = ''
        Sole skills source of truth: the flake `inputs.skills` store path.
        Home Manager only symlinks this path to ~/.config/omp/agent/skills,
        the directory the omp harness reads. Bezel does not invent a second tree.
        Do not point this at a second in-repo skills/ copy.
      '';
    };
  };

  config = lib.mkIf config.programs.bezel.enable {
    home.packages = [ config.programs.bezel.package ] ++ config.programs.bezel.extraPackages;

    xdg.configFile."omp/agent/skills".source = config.programs.bezel.skillsSource;
  };
}
