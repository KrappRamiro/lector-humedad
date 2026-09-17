# A note on why this file fights with pre-built binaries
# --------------------------------------------------------
# `cargo run` downloads pre-built ESP32 tools (a C compiler, clang, etc.)
# that were built for a normal Linux distro like Ubuntu.

# Those tools expect to find their supporting library files
# (things like "libxml2", the XML parsing library)
# in standard shared folders like /usr/lib.
#
# NixOS doesn't have those folders, every library lives in its own private folder
# under /nix/store instead, so a foreign, pre-built binary
# can't find anything it needs unless we point it there by hand.
#
# There are two different ways a program can go looking for a library, and
# each one needed its own fix below:
#   1. Normal startup: the "nix-ld" trick lets foreign binaries start up and find common libraries automatically.
#
#   2. `dlopen`: some programs (like Rust's `bindgen`, which reads the C compiler's headers to generate Rust code)
#      load a library manually, by name, while already running.
#      This bypasses nix-ld entirely, so we have to hand it the library locations ourselves,
#      via the LD_LIBRARY_PATH environment variable set in `enterShell` below.
#
{
  pkgs,
  inputs,
  ...
}:
let
  # Tell fenix to grab the latest nightly compiler
  # and include rust-src (needed for the ESP32) and various rust tools
  rustToolchain = inputs.fenix.packages.${pkgs.stdenv.system}.latest.withComponents [
    "cargo"
    "rustc"
    "rust-src"
    "rust-analyzer"
    "clippy"
    "rustfmt"
  ];
  # Workaround to let esp-clang find libxml2 on Nix.
  # nixpkgs ships libxml2.so.16 , but esp-clang expects the file to be named libxml2.so.2
  libxml2Compat = pkgs.runCommand "libxml2-compat" { } ''
    mkdir -p $out/lib
    ln -s ${pkgs.libxml2.out}/lib/libxml2.so $out/lib/libxml2.so.2
  '';
in
{
  packages = [
    # Happy news, standard Rustup handles RISC-V natively
    rustToolchain

    # provided via Nix so you don't have to wait 20 minutes
    # for `cargo install cargo-espflash espflash ldproxy`
    pkgs.espflash
    pkgs.ldproxy
    pkgs.cargo-generate

    # ESP-IDF C toolchain prerequisites
    # (required because esp-idf-sys compiles C code under the hood)
    # See https://esp-rs.github.io/std-training/02_2_software.html#debianubuntu
    pkgs.python3
    pkgs.cmake
    pkgs.ninja
    pkgs.git
    pkgs.pkg-config
    pkgs.flex
    pkgs.bison
    pkgs.gperf
    pkgs.ccache
    pkgs.libffi
    pkgs.openssl
    pkgs.libusb1

    # The next three packages exist purely to be found via LD_LIBRARY_PATH in `enterShell` below.
    #  see the big comment at the top of this file for why.
    #
    # They're all libraries that the pre-built esp-clang compiler
    # needs while it's being loaded by Rust's `bindgen` tool.
    pkgs.stdenv.cc.cc.lib # provides libstdc++
    pkgs.libxml2 # provides libxml2
    pkgs.zlib # provides libz
  ];

  enterShell = ''
    # bindgen loads esp-clang's libclang.so by hand at build time
    # (not at normal program startup), which bypasses nix-ld.
    # To fix that, we list the libraries it needs on LD_LIBRARY_PATH ourselves.
    export LD_LIBRARY_PATH="${libxml2Compat}/lib:${pkgs.stdenv.cc.cc.lib}/lib:${pkgs.zlib}/lib:$LD_LIBRARY_PATH"

    # Some recent Espressif esp-clang toolchain builds ship without a
    # libclang.so at all (see https://github.com/espressif/llvm-project/issues/108).
    # When esp-idf-sys detects that, it falls back to looking for a `~/.espup/esp-clang`
    # symlink (created by the `espup` CLI tool, which we don't use here) and otherwise
    # leaves LIBCLANG_PATH untouched.
    # That problem is fixed with this line:
    export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"

    echo "🦀 Rust Toolchain provided by Nix (fenix)!"
    rustc --version
  '';

  # See full reference at https://devenv.sh/reference/options/
}
