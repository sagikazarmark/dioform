{ pkgs, ... }:

{
  dotenv.enable = true;

  dagger = {
    enable = true;
    version = "v1.0.0-beta.15";

    dang.enable = true;
  };

  # Required by arborium
  env.CC_wasm32_unknown_unknown = "${pkgs.llvmPackages.clang-unwrapped}/bin/clang";

  packages = with pkgs; [
    lld

    cargo-audit
    cargo-deny
    cargo-release
    cargo-watch

    wasm-pack
  ];

  dioxus = {
    enable = true;

    cargoLock = ./demo/Cargo.lock;
  };

  worker-build = {
    enable = true;

    cargoLock = ./demo/Cargo.lock;
  };

  languages = {
    rust = {
      enable = true;
      channel = "stable";
      targets = [ "wasm32-unknown-unknown" ];
    };

    javascript = {
      enable = true;
      npm.enable = true;
    };

    tailwindcss = {
      enable = true;
    };
  };
}
