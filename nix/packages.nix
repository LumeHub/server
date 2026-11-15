{inputs, ...}: {
    perSystem = {pkgs, ...}: let
        craneLib = inputs.crane.mkLib pkgs;

        serverMeta = craneLib.crateNameFromCargoToml {
            cargoToml = ../crates/server/Cargo.toml;
        };

        # Rust sources plus files embedded at compile time (migrations, configs).
        src = pkgs.lib.cleanSourceWith {
            src = ../.;
            filter = path: type:
                (craneLib.filterCargoSources path type)
                || (builtins.match ".*\\.(sql|toml)$" path != null);
            name = "source";
        };

        commonArgs = serverMeta // {inherit src;};

        # Deps for the binary only; check/test artifacts live in checkArtifacts.
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs
            // {
                doCheck = false;
                cargoCheckCommand = "true";
            });

        checkArtifacts = craneLib.buildDepsOnly (commonArgs // {inherit cargoArtifacts;});

        lumehub-server = pkgs.lib.makeOverridable ({withGoogle ? true}:
            craneLib.buildPackage (commonArgs
            // {
                inherit cargoArtifacts;
                doCheck = false;
            }
            // pkgs.lib.optionalAttrs (!withGoogle) {
                cargoExtraArgs = "--no-default-features";
            })) {};
    in {
        packages = {
            inherit lumehub-server;
            default = lumehub-server;
        };
        checks = {
            inherit lumehub-server;
            clippy = craneLib.cargoClippy (commonArgs
            // {
                cargoArtifacts = checkArtifacts;
                cargoClippyExtraArgs = "-- --deny warnings";
            });
            test = craneLib.cargoTest (commonArgs // {cargoArtifacts = checkArtifacts;});
        };
    };
}
