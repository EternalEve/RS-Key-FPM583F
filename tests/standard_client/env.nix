# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors

let
  locked = (builtins.fromJSON (builtins.readFile ../../flake.lock)).nodes.nixpkgs.locked;
  pkgs = import (builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/${locked.rev}.tar.gz";
    sha256 = locked.narHash;
  }) { };
in
pkgs.python3.withPackages (ps: [
  (ps.buildPythonPackage {
    pname = "fido2";
    version = "2.2.1";
    format = "wheel";
    src = pkgs.fetchurl {
      url = "https://files.pythonhosted.org/packages/71/84/198d99c3312557ef6121cf78c38281efe9b3bc88cba0e2c05446f38a024d/fido2-2.2.1-py3-none-any.whl";
      sha256 = "ed397da981b9ab133da6ead7309e41f924b566b749956129efe286fae097749f";
    };
    dependencies = [ ps.cryptography ];
    pythonImportsCheck = [ "fido2" ];
    # Upstream's device tests reset authenticators; this environment only installs the library.
    doCheck = false;
  })
  ps.pytest
])
