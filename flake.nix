{
  description = "Console Mode - A Rust-based gamescope session launcher with automatic display detection";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        console-mode = pkgs.rustPlatform.buildRustPackage {
          pname = "console-mode";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs = with pkgs; [
            edid-decode
          ];

          # Make edid-decode available at runtime
          postInstall = ''
            wrapProgram $out/bin/console-mode \
              --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.edid-decode ]}
          '';

          meta = with pkgs.lib; {
            description = "A Rust-based gamescope session launcher with automatic display detection";
            homepage = "https://github.com/yourusername/console-mode";
            license = with licenses; [ mit asl20 ];
            maintainers = [ ];
            platforms = platforms.linux;
          };
        };

      in {
        packages = {
          default = console-mode;
          console-mode = console-mode;
        };

        apps = {
          default = {
            type = "app";
            program = "${console-mode}/bin/console-mode";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustc
            cargo
            rust-analyzer
            rustfmt
            clippy
            edid-decode
          ];
        };
      }
    ) // {
      # Home Manager module
      homeManagerModules.default = import ./nix/home-manager-module.nix;
      homeManagerModules.console-mode = import ./nix/home-manager-module.nix;

      # NixOS module (for system-level configuration if needed)
      nixosModules.default = import ./nix/nixos-module.nix;
      nixosModules.console-mode = import ./nix/nixos-module.nix;
    };
}
