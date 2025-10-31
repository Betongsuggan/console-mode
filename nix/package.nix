{ lib
, rustPlatform
, pkg-config
, edid-decode
, makeWrapper
}:

rustPlatform.buildRustPackage {
  pname = "console-mode";
  version = "0.1.0";

  src = ../.;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    edid-decode
  ];

  # Make edid-decode available at runtime
  postInstall = ''
    wrapProgram $out/bin/console-mode \
      --prefix PATH : ${lib.makeBinPath [ edid-decode ]}
  '';

  meta = with lib; {
    description = "A Rust-based gamescope session launcher with automatic display detection";
    homepage = "https://github.com/yourusername/console-mode";
    license = with licenses; [ mit asl20 ];
    maintainers = [ ];
    platforms = platforms.linux;
  };
}
