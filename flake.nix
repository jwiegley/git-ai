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
          jsonFormat = pkgs.formats.json { };

          # Build the config object, filtering out null values
          configFile = filterAttrs (n: v: v != null) {
            git_path =
              if cfg.settings.gitPath != null
              then cfg.settings.gitPath
              else "${pkgs.git}/bin/git";
            prompt_storage = cfg.settings.promptStorage;
            api_base_url = cfg.settings.apiBaseUrl;
            exclude_prompts_in_repositories = cfg.settings.excludePromptsInRepositories;
            allow_repositories = cfg.settings.allowRepositories;
            exclude_repositories = cfg.settings.excludeRepositories;
            telemetry_oss = cfg.settings.telemetryOss;
            telemetry_enterprise_dsn = cfg.settings.telemetryEnterpriseDsn;
            disable_version_checks = cfg.settings.disableVersionChecks;
            disable_auto_updates = cfg.settings.disableAutoUpdates;
            update_channel = cfg.settings.updateChannel;
            feature_flags =
              if cfg.settings.featureFlags != { }
              then cfg.settings.featureFlags
              else null;
          };

          # Generate the config file in the Nix store
          configJsonFile = jsonFormat.generate "git-ai-config.json" configFile;
        in
        {
          options.programs.git-ai = {
            enable = mkEnableOption "git-ai - AI-powered Git tracking";

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = literalExpression "inputs.git-ai.packages.\${pkgs.system}.default";
              description = "The git-ai package to use.";
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

            settings = {
              gitPath = mkOption {
                type = types.nullOr types.str;
                default = null;
                description = ''
                  Path to the git binary. If not specified, defaults to the
                  git package from nixpkgs.
                '';
              };

              promptStorage = mkOption {
                type = types.nullOr (types.enum [ "default" "notes" "local" ]);
                default = null;
                description = ''
                  Prompt storage mode:
                  - "default": Messages uploaded via CAS API
                  - "notes": Messages stored in git notes
                  - "local": Messages only stored in sqlite (not in notes, not uploaded)
                '';
              };

              apiBaseUrl = mkOption {
                type = types.nullOr types.str;
                default = null;
                description = ''
                  API base URL for git-ai services.
                  Defaults to "https://usegitai.com" if not specified.
                '';
              };

              excludePromptsInRepositories = mkOption {
                type = types.nullOr (types.listOf types.str);
                default = null;
                example = [ "https://github.com/private/*" "*" ];
                description = ''
                  List of repository URL patterns (globs) to exclude from prompt sharing.
                  Use "*" to exclude all repositories.
                '';
              };

              allowRepositories = mkOption {
                type = types.nullOr (types.listOf types.str);
                default = null;
                example = [ "https://github.com/myorg/*" ];
                description = ''
                  List of repository URL patterns (globs) to allow.
                  If empty or null, all repositories are allowed (unless excluded).
                '';
              };

              excludeRepositories = mkOption {
                type = types.nullOr (types.listOf types.str);
                default = null;
                example = [ "https://github.com/private/*" ];
                description = ''
                  List of repository URL patterns (globs) to exclude from git-ai tracking.
                  Exclusions take precedence over allow list.
                '';
              };

              telemetryOss = mkOption {
                type = types.nullOr (types.enum [ "on" "off" ]);
                default = null;
                description = ''
                  OSS telemetry setting. Set to "off" to disable telemetry.
                '';
              };

              telemetryEnterpriseDsn = mkOption {
                type = types.nullOr types.str;
                default = null;
                description = ''
                  Enterprise telemetry DSN for custom telemetry endpoints.
                '';
              };

              disableVersionChecks = mkOption {
                type = types.nullOr types.bool;
                default = null;
                description = ''
                  Whether to disable version checks.
                '';
              };

              disableAutoUpdates = mkOption {
                type = types.nullOr types.bool;
                default = null;
                description = ''
                  Whether to disable automatic updates.
                '';
              };

              updateChannel = mkOption {
                type = types.nullOr (types.enum [ "latest" "next" ]);
                default = null;
                description = ''
                  Update channel: "latest" for stable releases, "next" for pre-releases.
                '';
              };

              featureFlags = mkOption {
                type = jsonFormat.type;
                default = { };
                example = { rewrite_stash = true; };
                description = ''
                  Feature flags as a JSON-compatible attribute set.
                  See git-ai documentation for available flags.
                '';
              };
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

                    # Copy config.json from store (allows user to override later if needed)
                    # Only copy if the file doesn't exist or is a symlink (from previous Nix activation)
                    if [ ! -f "$user_home/.git-ai/config.json" ] || [ -L "$user_home/.git-ai/config.json" ]; then
                      cp -f ${configJsonFile} "$user_home/.git-ai/config.json"
                      chmod 644 "$user_home/.git-ai/config.json"
                      chown "$user" "$user_home/.git-ai/config.json" 2>/dev/null || true
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
          jsonFormat = pkgs.formats.json { };

          # Build the config object, filtering out null values
          # We use explicit null checks since Nix 'or' only works for attribute access
          configFile = filterAttrs (n: v: v != null) {
            git_path =
              if cfg.settings.gitPath != null
              then cfg.settings.gitPath
              else "${pkgs.git}/bin/git";
            prompt_storage = cfg.settings.promptStorage;
            api_base_url = cfg.settings.apiBaseUrl;
            exclude_prompts_in_repositories = cfg.settings.excludePromptsInRepositories;
            allow_repositories = cfg.settings.allowRepositories;
            exclude_repositories = cfg.settings.excludeRepositories;
            telemetry_oss = cfg.settings.telemetryOss;
            telemetry_enterprise_dsn = cfg.settings.telemetryEnterpriseDsn;
            disable_version_checks = cfg.settings.disableVersionChecks;
            disable_auto_updates = cfg.settings.disableAutoUpdates;
            update_channel = cfg.settings.updateChannel;
            feature_flags =
              if cfg.settings.featureFlags != { }
              then cfg.settings.featureFlags
              else null;
          };
        in
        {
          options.programs.git-ai = {
            enable = mkEnableOption "git-ai - AI-powered Git tracking";

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = literalExpression "inputs.git-ai.packages.\${pkgs.system}.default";
              description = "The git-ai package to use.";
            };

            installHooks = mkOption {
              type = types.bool;
              default = true;
              description = ''
                Whether to run 'git-ai install-hooks' on activation.
                This sets up IDE and agent integration hooks.
              '';
            };

            settings = {
              gitPath = mkOption {
                type = types.nullOr types.str;
                default = null;
                description = ''
                  Path to the git binary. If not specified, defaults to the
                  git package from nixpkgs.
                '';
              };

              promptStorage = mkOption {
                type = types.nullOr (types.enum [ "default" "notes" "local" ]);
                default = null;
                description = ''
                  Prompt storage mode:
                  - "default": Messages uploaded via CAS API
                  - "notes": Messages stored in git notes
                  - "local": Messages only stored in sqlite (not in notes, not uploaded)
                '';
              };

              apiBaseUrl = mkOption {
                type = types.nullOr types.str;
                default = null;
                description = ''
                  API base URL for git-ai services.
                  Defaults to "https://usegitai.com" if not specified.
                '';
              };

              excludePromptsInRepositories = mkOption {
                type = types.nullOr (types.listOf types.str);
                default = null;
                example = [ "https://github.com/private/*" "*" ];
                description = ''
                  List of repository URL patterns (globs) to exclude from prompt sharing.
                  Use "*" to exclude all repositories.
                '';
              };

              allowRepositories = mkOption {
                type = types.nullOr (types.listOf types.str);
                default = null;
                example = [ "https://github.com/myorg/*" ];
                description = ''
                  List of repository URL patterns (globs) to allow.
                  If empty or null, all repositories are allowed (unless excluded).
                '';
              };

              excludeRepositories = mkOption {
                type = types.nullOr (types.listOf types.str);
                default = null;
                example = [ "https://github.com/private/*" ];
                description = ''
                  List of repository URL patterns (globs) to exclude from git-ai tracking.
                  Exclusions take precedence over allow list.
                '';
              };

              telemetryOss = mkOption {
                type = types.nullOr (types.enum [ "on" "off" ]);
                default = null;
                description = ''
                  OSS telemetry setting. Set to "off" to disable telemetry.
                '';
              };

              telemetryEnterpriseDsn = mkOption {
                type = types.nullOr types.str;
                default = null;
                description = ''
                  Enterprise telemetry DSN for custom telemetry endpoints.
                '';
              };

              disableVersionChecks = mkOption {
                type = types.nullOr types.bool;
                default = null;
                description = ''
                  Whether to disable version checks.
                '';
              };

              disableAutoUpdates = mkOption {
                type = types.nullOr types.bool;
                default = null;
                description = ''
                  Whether to disable automatic updates.
                '';
              };

              updateChannel = mkOption {
                type = types.nullOr (types.enum [ "latest" "next" ]);
                default = null;
                description = ''
                  Update channel: "latest" for stable releases, "next" for pre-releases.
                '';
              };

              featureFlags = mkOption {
                type = jsonFormat.type;
                default = { };
                example = { rewrite_stash = true; };
                description = ''
                  Feature flags as a JSON-compatible attribute set.
                  See git-ai documentation for available flags.
                '';
              };
            };
          };

          config = mkIf cfg.enable {
            # Add git-ai to user packages
            home.packages = [ cfg.package ];

            # Create config directory and file
            home.file.".git-ai/config.json" = {
              source = jsonFormat.generate "git-ai-config.json" configFile;
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
