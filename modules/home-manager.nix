{ skills }:
{
  config,
  lib,
  pkgs,
  ...
}:

# Skills flake input is the sole SoT. This module copies that tree into each
# harness's user directory as regular files. Do not vendor a second skills/
# tree in Bezel. Nushell and Nickel are on PATH so the skill bodies can run.
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
        ++ lib.optional (pkgs ? nushell) pkgs.nushell
        ++ lib.optional (pkgs ? nickel) pkgs.nickel;
      defaultText = lib.literalExpression "[ pkgs.jev-router pkgs.cursor-agent-jev pkgs.grok-build-jev pkgs.nushell pkgs.nickel ]";
      description = "Additional Bezel packages on PATH (jev-router, cursor-agent-jev, grok-build-jev, and the pinned Nushell and Nickel used by skills).";
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

    antigravity.enable = lib.mkEnableOption "Antigravity (agy) skills registration";
    # When on, activation registers ~/.agents/skills in
    # ~/.gemini/config/skills.json. That points agy at the regular-file tree
    # this module already maintains: no fifth copy and no symlinks. The helper
    # preserves unrelated entries, leaves a registered file byte- and
    # mtime-identical, and refuses to follow a symlink or rewrite bad JSON.
  };

  config = lib.mkIf config.programs.bezel.enable {
    home.packages = [ config.programs.bezel.package ] ++ config.programs.bezel.extraPackages;

    # Regular files, one tree per harness. Matching bytes stay in place,
    # including their mode. Different bytes or a symlinked managed file fail
    # this activation: it does not pass --force, and the existing file is left
    # unchanged. lib.hm is Home Manager's module library.
    #
    # Antigravity reads ~/.gemini/config/skills.json. When enabled, register
    # the regular-file ~/.agents/skills tree this module already writes; never
    # a fifth copy and never a symlink. The helper is drift-safe and fails
    # closed on symlinks or malformed JSON.
    home.activation =
      let
        dag = lib.hm.dag or (throw "programs.bezel needs Home Manager (lib.hm.dag)");
      in
      {
        bezelUserSkills = dag.entryAfter [ "writeBoundary" ] ''
          ${pkgs.bash}/bin/bash ${../scripts}/install-user-skills.sh ${
            lib.escapeShellArg (toString config.programs.bezel.skillsSource)
          } ${lib.escapeShellArg config.home.homeDirectory}
        '';
      }
      // lib.optionalAttrs config.programs.bezel.antigravity.enable {
        bezelAgySkills = dag.entryAfter [ "writeBoundary" "bezelUserSkills" ] ''
          PYTHON=${pkgs.python3}/bin/python3 \
            ${pkgs.bash}/bin/bash ${../scripts}/install-agy-skills-config.sh ${
              lib.escapeShellArg config.home.homeDirectory
            }
        '';
      };
  };
}
