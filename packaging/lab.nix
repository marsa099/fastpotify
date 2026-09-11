# Standalone, pinned Linux test edition. Does not alter the default package.
# Build: nix build --impure --file packaging/lab.nix
# Install: nix profile install --impure --file packaging/lab.nix
{
  system ? builtins.currentSystem,
  fork ? builtins.getFlake "github:marsa099/fastpotify/7dfde7013fafe8abc754fd9cd47a75ebd5566e8e",
}:
let
  pkgs = import fork.inputs.nixpkgs { inherit system; };
  base = fork.packages.${system}.fastpotify;
  desktop = pkgs.makeDesktopItem {
    name = "fastpotify-lab";
    desktopName = "Fastpotify Lab";
    genericName = "Spotify fork testing";
    comment = "Vim-enabled test edition with a separate profile";
    exec = "fastpotify-lab";
    icon = "fastpotify-lab";
    startupWMClass = "fastpotify-lab";
    categories = [
      "AudioVideo"
      "Audio"
      "Player"
    ];
    # Deliberately no URI associations: the original remains the link handler.
  };
in
assert pkgs.stdenv.hostPlatform.isLinux;
base.overrideAttrs (old: {
  pname = "fastpotify-lab";
  postPatch = (old.postPatch or "") + ''
    substituteInPlace src/paths.rs \
      --replace-fail '"fastpotify"' '"fastpotify-lab"' \
      --replace-fail '"fastpotify-config"' '"fastpotify-lab-config"' \
      --replace-fail '"fastpotify-state"' '"fastpotify-lab-state"' \
      --replace-fail '"fastpotify-cache"' '"fastpotify-lab-cache"'
    substituteInPlace src/single_instance.rs \
      --replace-fail 'rocks.fastpotify.Instance' 'rocks.fastpotify.Lab.Instance' \
      --replace-fail '/rocks/fastpotify/Instance' '/rocks/fastpotify/Lab/Instance' \
      --replace-fail 'org.mpris.MediaPlayer2.fastpotify' 'org.mpris.MediaPlayer2.fastpotify_lab'
    substituteInPlace src/mpris.rs \
      --replace-fail '"fastpotify"' '"fastpotify_lab"' \
      --replace-fail '.identity("Fastpotify")' '.identity("Fastpotify Lab")'
    # MPRIS bus suffix uses an underscore; desktop file uses a hyphen.
    substituteInPlace src/mpris.rs \
      --replace-fail '        "fastpotify_lab"' '        "fastpotify-lab"'
    substituteInPlace src/main.rs \
      --replace-fail '"Fastpotify"' '"Fastpotify Lab"' \
      --replace-fail '.with_app_id("fastpotify")' '.with_app_id("fastpotify-lab")'
    # Update both the idle title and its default-device snapshot expectation.
    substituteInPlace src/app.rs \
      --replace-fail '"Fastpotify"' '"Fastpotify Lab"' \
      --replace-fail 'format!("{} - Fastpotify", now.title)' 'format!("{} - Fastpotify Lab", now.title)' \
      --replace-fail 'format!("{} - {}", now.subtitle, now.title)' 'format!("{} - {} - Fastpotify Lab", now.subtitle, now.title)'
    substituteInPlace src/tray.rs \
      --replace-fail '"fastpotify"' '"fastpotify-lab"' \
      --replace-fail '"Fastpotify"' '"Fastpotify Lab"' \
      --replace-fail 'Show or hide Fastpotify' 'Show or hide Fastpotify Lab'
    substituteInPlace src/settings.rs \
      --replace-fail 'device_name: "Fastpotify".to_string()' 'device_name: "Fastpotify Lab".to_string()'
    # Credentials already use a hash of the state path as their profile key.
    # Keeping the credential service but changing AppDirs isolates all grants.
    substituteInPlace packaging/icons/fastpotify.svg \
      --replace-fail '#1ed760' '#a78bfa' \
      --replace-fail '#0a140e' '#21133d'
  '';
  postInstall = ''
    mv "$out/bin/fastpotify" "$out/bin/fastpotify-lab"
    install -Dm644 ${desktop}/share/applications/fastpotify-lab.desktop \
      "$out/share/applications/fastpotify-lab.desktop"
    install -Dm644 packaging/icons/fastpotify.svg \
      "$out/share/icons/hicolor/scalable/apps/fastpotify-lab.svg"
  '';
  postFixup =
    builtins.replaceStrings [ "$out/bin/fastpotify" ] [ "$out/bin/fastpotify-lab" ]
      old.postFixup;
  meta = old.meta // {
    description = "Fastpotify Lab: isolated Vim-navigation test edition";
    mainProgram = "fastpotify-lab";
    platforms = pkgs.lib.platforms.linux;
  };
})
