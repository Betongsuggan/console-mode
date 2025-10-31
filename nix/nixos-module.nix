{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.programs.console-mode;
  console-mode-pkg = pkgs.callPackage ./package.nix { };
in {
  options.programs.console-mode = {
    enable = mkEnableOption "Console Mode - gamescope session launcher";

    package = mkOption {
      type = types.package;
      default = console-mode-pkg;
      defaultText = literalExpression "pkgs.console-mode";
      description = "The console-mode package to use.";
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    # Ensure required runtime dependencies are available
    environment.sessionVariables = {
      # Make edid-decode accessible
      PATH = [ "${pkgs.edid-decode}/bin" ];
    };
  };
}
