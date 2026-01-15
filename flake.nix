{
  description = "git-ai - AI-powered Git tracking and intelligence for code repositories";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        # Build the git-ai binary using rustPlatform
        git-ai-unwrapped = pkgs.rustPlatform.buildRustPackage {
          pname = "git-ai";
          version = "1.0.35";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          # Native build inputs needed for rusqlite with bundled SQLite
          nativeBuildInputs = with pkgs; [
            pkg-config
            rustPlatform.bindgenHook  # For rusqlite bundled builds
          ];

          # Build inputs for runtime dependencies
          buildInputs = with pkgs; [
            # rusqlite bundled mode compiles its own SQLite, but needs these headers
            sqlite
          ] ++ lib.optionals stdenv.hostPlatform.isDarwin [
            # macOS-specific dependencies
            libiconv
            apple-sdk_15
          ];

          # Tests require git and specific setup
          doCheck = false;

          meta = with pkgs.lib; {
            description = "AI-powered Git wrapper that tracks AI-generated code changes";
            homepage = "https://github.com/acunniffe/git-ai";
            license = licenses.gpl3Plus;
            maintainers = [ ];
            mainProgram = "git-ai";
            platforms = platforms.unix;
          };
        };

        # Wrapped version that sets up the git-ai environment properly
        git-ai-wrapped = pkgs.writeShellScriptBin "git-ai" ''
          # Ensure config directory exists
          mkdir -p "$HOME/.git-ai"

          # Create config.json if it doesn't exist
          if [ ! -f "$HOME/.git-ai/config.json" ]; then
            # Find the system git (not our wrapper)
            GIT_PATH="${pkgs.git}/bin/git"
            cat > "$HOME/.git-ai/config.json" <<EOF
          {
            "git_path": "$GIT_PATH"
          }
          EOF
          fi

          # Execute git-ai with all arguments
          exec ${git-ai-unwrapped}/bin/git-ai "$@"
        '';

        # Wrapper for git command that preserves argv[0] as "git"
        # This is critical: when symlinked as "git", the wrapper must set argv[0]
        # to "git" so the Rust binary routes to handle_git() instead of handle_git_ai()
        git-wrapper = pkgs.writeShellScriptBin "git" ''
          # Ensure config directory exists
          mkdir -p "$HOME/.git-ai"

          # Create config.json if it doesn't exist
          if [ ! -f "$HOME/.git-ai/config.json" ]; then
            # Find the system git (not our wrapper)
            GIT_PATH="${pkgs.git}/bin/git"
            cat > "$HOME/.git-ai/config.json" <<EOF
          {
            "git_path": "$GIT_PATH"
          }
          EOF
          fi

          # Execute git-ai with argv[0] set to "git" to trigger passthrough mode
          # The -a flag ensures argv[0] is "git" regardless of the actual binary path
          exec -a git ${git-ai-unwrapped}/bin/git-ai "$@"
        '';

        # Create git-og wrapper that bypasses git-ai and calls real git directly
        # This is needed because git interprets argv[0] as a subcommand
        git-og = pkgs.writeShellScriptBin "git-og" ''
          exec ${pkgs.git}/bin/git "$@"
        '';

        # Package without git wrapper - for Home Manager / environments with existing git
        git-ai-minimal = pkgs.symlinkJoin {
          name = "git-ai-minimal-${git-ai-unwrapped.version}";
          paths = [ git-ai-wrapped git-ai-unwrapped git-og ];

          meta = git-ai-unwrapped.meta // {
            description = git-ai-unwrapped.meta.description + " (without git wrapper)";
          };
        };

        # Create a complete package with git wrapper (for standalone use)
        # The git-wrapper script ensures argv[0] is "git" when invoked as git
        git-ai-package = pkgs.symlinkJoin {
          name = "git-ai-${git-ai-unwrapped.version}";
          paths = [ git-ai-wrapped git-wrapper git-ai-unwrapped git-og ];

          meta = git-ai-unwrapped.meta // {
            description = git-ai-unwrapped.meta.description + " (with git wrapper)";
          };
        };

      in
      {
        # Development shell with full Rust toolchain
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            # Rust development tools
            rustc
            cargo
            clippy
            rustfmt
            rust-analyzer

            # Build dependencies
            pkg-config

            # Runtime dependencies for testing
            git
            sqlite

            # Useful development tools
            cargo-edit      # cargo add, cargo rm, cargo upgrade
            cargo-watch     # Auto-rebuild on file changes
            cargo-expand    # Show macro expansions
          ] ++ lib.optionals stdenv.hostPlatform.isDarwin [
            libiconv
            apple-sdk_15
          ];

          # Environment variables for development
          shellHook = ''
            echo "🦀 git-ai development environment"
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build          - Build the project"
            echo "  cargo test           - Run tests"
            echo "  cargo run            - Run git-ai"
            echo ""

            # Set up environment for development
            export RUST_BACKTRACE=1
            export RUST_LOG=debug
          '';
        };

        # Main packages
        packages = {
          # Unwrapped binary (just the git-ai executable)
          unwrapped = git-ai-unwrapped;

          # Wrapped version with helper scripts
          wrapped = git-ai-wrapped;

          # Minimal package without git symlink (for Home Manager/environments with existing git)
          minimal = git-ai-minimal;

          # Complete package with git/git-og symlinks (for standalone use)
          default = git-ai-package;

          # Alias for clarity
          git-ai = git-ai-package;
        };

        # Make app available for `nix run`
        apps.default = flake-utils.lib.mkApp {
          drv = git-ai-package;
          exePath = "/bin/git-ai";
        };

        # Formatter for `nix fmt`
        formatter = pkgs.nixpkgs-fmt;
      }
    ) // {
      # System-independent outputs

      # Overlay for importing into other flakes
      overlays.default = final: prev: {
        git-ai = self.packages.${prev.system}.default;
        git-ai-unwrapped = self.packages.${prev.system}.unwrapped;
      };

      # NixOS module for system integration
      nixosModules.default = { config, lib, pkgs, ... }:
        with lib;
        let
          cfg = config.programs.git-ai;
        in
        {
          options.programs.git-ai = {
            enable = mkEnableOption "git-ai - AI-powered Git tracking";

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = literalExpression "inputs.git-ai.packages.\${pkgs.system}.default";
              description = "The git-ai package to use";
            };

            installHooks = mkOption {
              type = types.bool;
              default = true;
              description = ''
                Whether to run 'git-ai install-hooks' on system activation.
                This sets up IDE and agent integration hooks.
              '';
            };

            setGitAlias = mkOption {
              type = types.bool;
              default = true;
              description = ''
                Whether to make 'git' command use git-ai wrapper.
                When enabled, git-ai is placed before regular git in PATH.
                The original git is still accessible via 'git-og'.
              '';
            };
          };

          config = mkIf cfg.enable {
            # Add git-ai to system packages
            environment.systemPackages = [ cfg.package ];

            # Set up system-wide configuration on activation
            system.activationScripts.git-ai = mkIf cfg.installHooks (
              stringAfter [ "users" ] ''
                # Run install-hooks for all users with home directories
                for user_home in /home/* /Users/* /root; do
                  if [ -d "$user_home" ]; then
                    user=$(basename "$user_home")

                    # Create config directory
                    mkdir -p "$user_home/.git-ai"

                    # Create config.json if it doesn't exist
                    if [ ! -f "$user_home/.git-ai/config.json" ]; then
                      cat > "$user_home/.git-ai/config.json" <<EOF
                {
                  "git_path": "${pkgs.git}/bin/git"
                }
                EOF
                      chown -R "$user" "$user_home/.git-ai" 2>/dev/null || true
                    fi

                    # Install hooks (run as user if possible)
                    if command -v sudo >/dev/null 2>&1 && [ "$user" != "root" ]; then
                      sudo -u "$user" ${cfg.package}/bin/git-ai install-hooks 2>/dev/null || true
                    else
                      ${cfg.package}/bin/git-ai install-hooks 2>/dev/null || true
                    fi
                  fi
                done
              ''
            );
          };
        };

      # Home Manager module for user-level configuration
      homeManagerModules.default = { config, lib, pkgs, ... }:
        with lib;
        let
          cfg = config.programs.git-ai;
        in
        {
          options.programs.git-ai = {
            enable = mkEnableOption "git-ai - AI-powered Git tracking";

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = literalExpression "inputs.git-ai.packages.\${pkgs.system}.default";
              description = "The git-ai package to use";
            };

            installHooks = mkOption {
              type = types.bool;
              default = true;
              description = ''
                Whether to run 'git-ai install-hooks' on activation.
                This sets up IDE and agent integration hooks.
              '';
            };

            createConfig = mkOption {
              type = types.bool;
              default = true;
              description = ''
                Whether to create ~/.git-ai/config.json with git_path.
              '';
            };
          };

          config = mkIf cfg.enable {
            # Create config directory and file
            home.file.".git-ai/config.json" = mkIf cfg.createConfig {
              text = builtins.toJSON {
                git_path = "${pkgs.git}/bin/git";
              };
            };

            # Run install-hooks on activation
            home.activation.git-ai-install-hooks = mkIf cfg.installHooks (
              lib.hm.dag.entryAfter [ "writeBoundary" ] ''
                $DRY_RUN_CMD ${cfg.package}/bin/git-ai install-hooks || true
              ''
            );
          };
        };
    };
}
