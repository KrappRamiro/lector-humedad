# A note on why this file fights with pre-built binaries
# --------------------------------------------------------
# `cargo build` downloads pre-built ESP32 tools (the RISC-V GCC, esp-clang,
# cmake, ninja) that were built for a normal Linux distro like Ubuntu.
#
# Those tools expect to find their supporting library files
# (things like libstdc++ and zlib) in standard shared folders like /usr/lib.
#
# NixOS doesn't have those folders, every library lives in its own private folder
# under /nix/store instead, so a foreign, pre-built binary
# can't find anything it needs unless we point it there by hand.
#
# There are two different ways a program can go looking for a library, and
# each one needs its own fix:
#   1. Normal startup: nix-ld (enabled in the NixOS system config, not here)
#      lets foreign binaries start up and find common libraries automatically.
#
#   2. `dlopen`: Rust's `bindgen` (which reads the ESP-IDF C headers to generate
#      Rust code) loads libclang manually, by name, while already running.
#      nix-ld can't help with that, so we point bindgen at the libclang from
#      nixpkgs through LIBCLANG_PATH in `enterShell` below. Nix-built libraries
#      know where their own dependencies live, so nothing else is needed.
#
# Don't export LD_LIBRARY_PATH in this file. It leaks into every program started
# from this shell (editors, language servers like nixd) and makes Nix-built
# programs load the wrong libstdc++, which crashes them.
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
in
{
  packages = [
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
  ];

  enterShell = ''
    # Recent Espressif esp-clang toolchain builds (like esp-19.1.2, used by
    # ESP-IDF v5.5, and esp-21.1.3, used by v6.1) ship without a libclang.so at all
    # (see https://github.com/espressif/llvm-project/issues/108).
    # When esp-idf-sys detects that, it falls back to looking for a `~/.espup/esp-clang`
    # symlink (created by the `espup` CLI tool, which we don't use here) and otherwise
    # leaves LIBCLANG_PATH untouched.
    #
    # If a future ESP-IDF version picks an esp-clang that ships libclang.so again
    # and the build fails with "libxml2.so.2: cannot open shared object file",
    # bindgen is loading Espressif's libclang instead of this one.
    #
    # This line gives bindgen the libclang that esp-clang no longer ships:
    export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"

    echo "🦀 Rust Toolchain provided by Nix (fenix)!"
    rustc --version
  '';

  enterTest = ''
    # devenv test hides stdout when the script succeeds; send it to stderr so results stay visible
    exec 1>&2
    set -e
    cargo fmt --all -- --check
    cargo test-core --locked
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    cargo build --release --locked
  '';

  # See full reference at https://devenv.sh/reference/options/
}
