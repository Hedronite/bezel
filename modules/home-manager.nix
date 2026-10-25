{ skills }:
{
  config,
  lib,
  pkgs,
  ...
}:

# Skills flake input is the sole SoT. This module copies that tree into each
# harness's user directory as regular files. Do not vendor a second skills/
# tree in Bezel. Nushell is on PATH so the Lapis, Facet, and Geode skill
# bodies can run.
# `programs.omapi` is deprecated. It renames to `programs.bezel`.

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
      description = "Bezel package. The omp engine command is bezel-omp. omapi is a symlink for one release.";
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default =
        lib.optional (pkgs ? jev-router) pkgs.jev-router
        ++ lib.optional (pkgs ? cursor-agent-jev) pkgs.cursor-agent-jev
        ++ lib.optional (pkgs ? grok-build-jev) pkgs.grok-build-jev
        ++ lib.optional (pkgs ? nushell) pkgs.nushell;
      defaultText = lib.literalExpression "[ pkgs.jev-router pkgs.cursor-agent-jev pkgs.grok-build-jev pkgs.nushell ]";
      description = "Additional Bezel packages on PATH (jev-router, cursor-agent-jev, grok-build-jev, and the pinned Nushell used by skills).";
    };

    skillsSource = lib.mkOption {
      type = lib.types.path;
      default = skills;
      defaultText = lib.literalExpression "inputs.skills";
      description = ''
        Sole skills source of truth: the flake `inputs.skills` store path.
        Home Manager copies each skill directory as regular files into
        ~/.agents/skills, ~/.claude/skills, ~/.grok/skills, and ~/.cursor/skills.
        Bezel does not invent a second tree. Do not point this at a second
        in-repo skills/ copy.
      '';
    };
  };

  config = lib.mkIf config.programs.bezel.enable {
    home.packages = [ config.programs.bezel.package ] ++ config.programs.bezel.extraPackages;

    # Regular files, one tree per harness. A second activation leaves matching
    # bytes untouched. lib.hm is Home Manager's module library.
    home.activation.bezelUserSkills =
      let
        dag = lib.hm.dag or (throw "programs.bezel needs Home Manager (lib.hm.dag)");
      in
      dag.entryAfter [ "writeBoundary" ] ''
        ${pkgs.bash}/bin/bash ${../scripts}/install-user-skills.sh ${
          lib.escapeShellArg (toString config.programs.bezel.skillsSource)
        } ${lib.escapeShellArg config.home.homeDirectory}
      '';
  };
}
