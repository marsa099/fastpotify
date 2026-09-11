# Fastpotify Lab (Linux)

An opt-in, pinned edition of the Vim-navigation fork. The original package is
unchanged. Lab has a purple launcher icon and the display name **Fastpotify Lab**.

From a checkout of this fork:

```sh
nix build --impure --file packaging/lab.nix
nix profile install --impure --file packaging/lab.nix
```

Launch **Fastpotify Lab** from the application menu, or run `fastpotify-lab`.
Installation does not launch the application or rebuild NixOS. With a fresh
profile, enable **Vim keys** under **Settings → Keyboard** to test navigation.

## Side-by-side isolation

- Config, state and cache use `fastpotify-lab` instead of `fastpotify` under the
  normal XDG directories. Lab does not automatically copy an existing profile.
- Credential storage retains the existing Secret Service service name but uses
  a different profile key, derived from the Lab state path. Copying settings
  alone does **not** copy a current version's protected sign-in credentials.
- Window identity, single-instance D-Bus name, MPRIS name, tray identity and
  desktop filename are separate. Lab does not register Spotify URI associations.
- The default Spotify Connect device name is **Fastpotify Lab**. Keep it distinct
  when importing settings: the Connect device ID is derived from that name.

Both clients still use the same Spotify account when credentials are copied.
Account-side edits affect both clients, and Spotify playback restrictions still
apply. Local profile isolation is not account isolation.

## Updating or removing Lab

The source revision is pinned in `lab.nix`. To test another revision, update that
pin, rebuild and test the package, then reinstall it. Packaging substitutions use
`--replace-fail` so source changes cannot silently disable isolation.

Use `nix profile list` to find the installed Lab entry and
`nix profile remove <entry-name>` to remove it. This leaves its settings and
credentials intact and never removes the original app.

The package build runs the inherited Rust tests. No visible application is
started during the build. Only Linux packaging is supported by this recipe.
