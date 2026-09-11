//! Keyboard shortcuts.

use egui::{Key, Modifiers};

use crate::app::App;
use crate::model::{Action, Dialog, Page};

pub(super) const fn platform_shortcut<'a>(ctrl: &'a str, cmd: &'a str) -> &'a str {
    if cfg!(target_os = "macos") { cmd } else { ctrl }
}

pub(super) const SIDEBAR_SHORTCUT: &str = platform_shortcut("Ctrl+B", "Cmd+B");
pub(super) const QUIT_SHORTCUT: &str = platform_shortcut("Ctrl+Q", "Cmd+Q");
pub(super) const WINAMP_SHORTCUT: &str = platform_shortcut("Ctrl+M", "Cmd+Shift+M");
pub(super) const MILKDROP_SHORTCUT: &str = platform_shortcut("Ctrl+Shift+K", "Cmd+Shift+K");

pub fn handle(app: &mut App, ctx: &egui::Context) {
    let typing = ctx.memory(|memory| memory.focused().is_some());
    let mut actions = Vec::new();
    let vim = app.settings.vim_keys;
    let help = !typing && ctx.input_mut(|input| consume_shortcut_help(&mut input.events));
    if help {
        actions.push(Action::ShowDialog(Dialog::Shortcuts));
    }
    let blocked =
        typing || help || app.dialog.is_some() || app.show_devices || egui::Popup::is_any_open(ctx);
    if vim && !blocked {
        let (commands, prefix) =
            ctx.input_mut(|input| vim_commands(&mut input.events, input.time, app.navigation.g_at));
        actions.extend(commands.into_iter().map(Action::Navigate));
        actions.push(Action::VimPrefix(prefix));
    } else if app.navigation.g_at.is_some() {
        actions.push(Action::VimPrefix(None));
    }
    ctx.input_mut(|input| {
        let mut key = |modifiers: Modifiers, key: Key, action: Action| {
            if input.consume_key(modifiers, key) {
                actions.push(action);
            }
        };
        key(Modifiers::COMMAND, Key::F, Action::FocusSearch);
        key(Modifiers::COMMAND, Key::B, Action::ToggleSidebar);
        key(Modifiers::COMMAND, Key::Comma, Action::Open(Page::Settings));
        key(Modifiers::COMMAND, Key::Q, Action::Quit);
        // The platform's close key. macOS only closes a window from its
        // menu, which winit does not install, and the mini player has no
        // title bar for the system to close it by.
        key(Modifiers::COMMAND, Key::W, Action::CloseWindow);
        // winit installs its own macOS app menu, whose Hide item owns Cmd+H
        // before the window is offered the key.
        if cfg!(target_os = "macos") {
            key(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::H,
                Action::Open(Page::Home),
            );
        } else if !vim {
            key(Modifiers::COMMAND, Key::H, Action::Open(Page::Home));
        }
        // Ctrl+h/l belong to pane navigation in Vim mode, but remain entirely
        // untouched while an input has focus. Do not fall back to Home/Liked.
        if !vim || cfg!(target_os = "macos") {
            key(Modifiers::COMMAND, Key::L, Action::Open(Page::LikedSongs));
        }
        // Cmd+M minimises on macOS.
        if cfg!(target_os = "macos") {
            key(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::M,
                Action::ToggleWinampWindow,
            );
        } else {
            key(Modifiers::COMMAND, Key::M, Action::ToggleWinampWindow);
        }
        // Winamp's key for starting and stopping the visualisation plug-in.
        key(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::K,
            Action::ToggleWinampMilkdrop,
        );
        key(
            Modifiers::COMMAND,
            Key::Slash,
            Action::ShowDialog(Dialog::Shortcuts),
        );
        key(Modifiers::ALT, Key::ArrowLeft, Action::Back);
        key(Modifiers::ALT, Key::ArrowRight, Action::Forward);
        key(Modifiers::COMMAND, Key::ArrowLeft, Action::Previous);
        key(Modifiers::COMMAND, Key::ArrowRight, Action::Next);
        key(Modifiers::COMMAND, Key::ArrowUp, Action::VolumeBy(5));
        key(Modifiers::COMMAND, Key::ArrowDown, Action::VolumeBy(-5));
        key(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::A,
            Action::OpenUri("artist".into()),
        );
        key(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::B,
            Action::OpenUri("album".into()),
        );
        // Cmd+Shift+Q is Log Out, taken by the window server.
        if cfg!(target_os = "macos") {
            key(Modifiers::COMMAND, Key::U, Action::ToggleQueuePanel);
        } else {
            key(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::Q,
                Action::ToggleQueuePanel,
            );
        }
        if !typing {
            key(Modifiers::SHIFT, Key::ArrowLeft, Action::SeekBy(-10_000));
            key(Modifiers::SHIFT, Key::ArrowRight, Action::SeekBy(10_000));
            key(Modifiers::NONE, Key::Space, Action::TogglePlay);
            key(Modifiers::NONE, Key::M, Action::ToggleMute);
            key(Modifiers::NONE, Key::S, Action::ToggleShuffle);
            key(Modifiers::NONE, Key::R, Action::CycleRepeat);
            key(Modifiers::NONE, Key::Q, Action::ToggleQueuePanel);
            key(
                if vim {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
                Key::L,
                Action::ToggleLyricsPanel,
            );
            key(Modifiers::NONE, Key::Slash, Action::FocusSearch);
        }
    });
    if !typing
        && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::B))
        && let Some(now) = app.now_playing().filter(|now| !now.is_episode)
    {
        actions.push(Action::ToggleSaved(now.uri));
    }
    // Resolve the "open current artist/album" placeholders.
    for action in actions {
        match action {
            Action::OpenUri(kind) if kind == "artist" => {
                if let Some(id) = app
                    .now_playing()
                    .and_then(|now| now.artists.first().and_then(|artist| artist.id.clone()))
                {
                    app.actions.push(Action::Open(Page::Artist(id)));
                }
            }
            Action::OpenUri(kind) if kind == "album" => {
                if let Some(now) = app.now_playing() {
                    if let Some(id) = now.album_id {
                        app.actions.push(Action::Open(Page::Album(id)));
                    } else if let Some(id) = now.show_id {
                        app.actions.push(Action::Open(Page::Show(id)));
                    }
                }
            }
            other => app.actions.push(other),
        }
    }
    // Map mouse back and forward buttons to navigation.
    let (back, forward) = ctx.input(|input| {
        (
            input.pointer.button_pressed(egui::PointerButton::Extra1),
            input.pointer.button_pressed(egui::PointerButton::Extra2),
        )
    });
    if back {
        app.actions.push(Action::Back);
    }
    if forward {
        app.actions.push(Action::Forward);
    }
    if ctx.input(|input| input.key_pressed(Key::Escape)) {
        if app.dialog.is_some() {
            app.actions.push(Action::CloseDialog);
        } else if app.show_devices {
            app.show_devices = false;
        }
    }
}

/// Layouts can report ? as a named key, Shift+Slash, or only a text event.
/// Consume the slash event too so help cannot accidentally focus search.
fn consume_shortcut_help(events: &mut Vec<egui::Event>) -> bool {
    let text_question = events
        .iter()
        .any(|event| matches!(event, egui::Event::Text(text) if text == "?"));
    let question_key = |event: &egui::Event| match event {
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            !modifiers.ctrl
                && !modifiers.mac_cmd
                && !modifiers.command
                && !modifiers.alt
                && (*key == Key::Questionmark
                    || (*key == Key::Slash && (modifiers.shift || text_question)))
        }
        _ => false,
    };
    let requested = text_question || events.iter().any(question_key);
    if requested {
        events.retain(|event| {
            !question_key(event) && !matches!(event, egui::Event::Text(text) if text == "?")
        });
    }
    requested
}

/// Consume exact modifiers so Shift+G cannot become the first half of gg,
/// and command shortcuts never become bare navigation keys.
fn vim_commands(
    events: &mut Vec<egui::Event>,
    now: f64,
    prefix: Option<f64>,
) -> (Vec<super::navigation::Command>, Option<f64>) {
    use super::navigation::Command;
    let mut prefix = prefix.filter(|at| now - at <= 0.75);
    let mut commands = Vec::new();
    events.retain(|event| {
        let egui::Event::Key {
            key,
            pressed: true,
            repeat,
            modifiers,
            ..
        } = event
        else {
            return true;
        };
        if *modifiers == Modifiers::NONE && *key == Key::G {
            if !repeat {
                if prefix.take().is_some() {
                    commands.push(Command::First);
                } else {
                    prefix = Some(now);
                }
            }
            return false;
        }
        if *modifiers == Modifiers::NONE && *key == Key::H && !repeat && prefix.is_some() {
            prefix = None;
            commands.push(Command::Home);
            return false;
        }
        prefix = None;
        let command = if *modifiers == Modifiers::NONE {
            match key {
                Key::H => Some(Command::Left),
                Key::L => Some(Command::Right),
                Key::J => Some(Command::Down),
                Key::K => Some(Command::Up),
                Key::Enter => Some(Command::Play),
                Key::O => Some(Command::Open),
                Key::Escape => Some(Command::Clear),
                _ => None,
            }
        } else if *modifiers == Modifiers::SHIFT && *key == Key::G {
            Some(Command::Last)
        } else if modifiers.ctrl && !modifiers.alt && !modifiers.shift && !modifiers.mac_cmd {
            match key {
                Key::D => Some(Command::HalfDown),
                Key::U => Some(Command::HalfUp),
                Key::H => Some(Command::PaneLeft),
                Key::L => Some(Command::PaneRight),
                _ => None,
            }
        } else {
            None
        };
        if let Some(command) = command {
            commands.push(command);
            false
        } else {
            true
        }
    });
    (commands, prefix)
}

pub const SHORTCUTS: &[(&str, &str)] = &[
    ("Space", "Play or pause"),
    (
        platform_shortcut("Ctrl+←  /  Ctrl+→", "Cmd+←  /  Cmd+→"),
        "Previous or next",
    ),
    ("Shift+←  /  Shift+→", "Seek 10 seconds"),
    (
        platform_shortcut("Ctrl+↑  /  Ctrl+↓", "Cmd+↑  /  Cmd+↓"),
        "Volume up or down",
    ),
    ("M", "Mute or unmute"),
    ("B", "Like or unlike the playing song"),
    ("S", "Toggle shuffle"),
    ("R", "Cycle repeat"),
    ("Q", "Show the queue"),
    ("L (Shift+L with Vim keys)", "Show the lyrics"),
    (
        "h / j / k / l",
        "Vim keys: spatial card selection (h/l switch panes in track lists)",
    ),
    ("j / k", "Vim keys: select next / previous row"),
    ("h (Home left edge)", "Vim keys: focus the sidebar"),
    ("gh", "Vim keys: open Home"),
    (
        "l (sidebar)",
        "Vim keys: open selected entry and focus its content",
    ),
    (
        "Control+h / Control+l",
        "Vim keys: switch panes (not in inputs; replaces Home/Liked Songs on Linux/Windows)",
    ),
    (
        "gg / Shift+G",
        "Vim keys: first / last row (loads remaining pages)",
    ),
    ("Control+d / Control+u", "Vim keys: half-page down / up"),
    (
        "Enter / o",
        "Vim keys: open card; in lists, play / open album",
    ),
    ("Esc", "Vim keys: clear selection"),
    (platform_shortcut("Ctrl+F  or  /", "Cmd+F  or  /"), "Search"),
    (SIDEBAR_SHORTCUT, "Show or hide the sidebar"),
    ("Alt+←  /  Alt+→", "Back or forward"),
    (platform_shortcut("Ctrl+H", "Cmd+Shift+H"), "Home"),
    (platform_shortcut("Ctrl+L", "Cmd+L"), "Liked Songs"),
    (
        platform_shortcut("Ctrl+Shift+A", "Cmd+Shift+A"),
        "Go to the playing artist",
    ),
    (
        platform_shortcut("Ctrl+Shift+B", "Cmd+Shift+B"),
        "Go to the playing album",
    ),
    (WINAMP_SHORTCUT, "Winamp mini player"),
    (MILKDROP_SHORTCUT, "MilkDrop, under the mini player"),
    ("F  or  double-click", "MilkDrop: fill the screen"),
    ("→  /  N", "MilkDrop: next preset"),
    ("←  /  P", "MilkDrop: previous preset"),
    ("L", "MilkDrop: keep this preset"),
    ("Esc", "MilkDrop: leave full screen, or close"),
    (platform_shortcut("Ctrl+,", "Cmd+,"), "Settings"),
    (
        platform_shortcut("Ctrl+/ or ?", "Cmd+/ or ?"),
        "Keyboard shortcuts",
    ),
    (platform_shortcut("Ctrl+W", "Cmd+W"), "Close the window"),
    (QUIT_SHORTCUT, "Quit"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppOptions;
    use crate::paths::AppDirs;
    use crate::settings::Settings;

    fn event(key: Key, modifiers: Modifiers, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers,
        }
    }

    #[test]
    fn vim_prefix_requires_two_presses_and_expires() {
        use super::super::navigation::Command;
        let (_, prefix) = vim_commands(&mut vec![event(Key::G, Modifiers::NONE, false)], 1.0, None);
        assert_eq!(prefix, Some(1.0));
        let (commands, _) =
            vim_commands(&mut vec![event(Key::G, Modifiers::NONE, true)], 1.1, prefix);
        assert!(commands.is_empty());
        let (commands, prefix) = vim_commands(
            &mut vec![event(Key::G, Modifiers::NONE, false)],
            1.2,
            prefix,
        );
        assert_eq!(commands, vec![Command::First]);
        assert_eq!(prefix, None);
        let (commands, prefix) = vim_commands(
            &mut vec![event(Key::G, Modifiers::NONE, false)],
            3.0,
            Some(1.0),
        );
        assert!(commands.is_empty());
        assert_eq!(prefix, Some(3.0));
        let (commands, prefix) = vim_commands(
            &mut vec![
                event(Key::J, Modifiers::NONE, false),
                event(Key::G, Modifiers::NONE, false),
            ],
            3.1,
            prefix,
        );
        assert_eq!(commands, vec![Command::Down]);
        assert_eq!(prefix, Some(3.1));
    }

    #[test]
    fn vim_modifiers_are_exact_and_unrelated_shortcuts_survive() {
        use super::super::navigation::Command;
        let mut events = vec![
            event(Key::G, Modifiers::SHIFT, false),
            event(Key::D, Modifiers::CTRL, false),
            event(Key::U, Modifiers::CTRL, false),
            event(Key::L, Modifiers::SHIFT, false),
            event(Key::L, Modifiers::COMMAND, false),
            event(Key::K, Modifiers::ALT, false),
        ];
        let (commands, prefix) = vim_commands(&mut events, 0.0, Some(0.0));
        assert_eq!(
            commands,
            vec![Command::Last, Command::HalfDown, Command::HalfUp]
        );
        assert_eq!(events.len(), 3);
        assert_eq!(prefix, None);
    }

    #[test]
    fn vim_pane_shortcuts_require_control_with_or_without_the_command_flag() {
        use super::super::navigation::Command;
        for modifiers in [Modifiers::CTRL, Modifiers::CTRL | Modifiers::COMMAND] {
            let mut events = vec![
                event(Key::H, modifiers, false),
                event(Key::L, modifiers, false),
            ];
            let (commands, _) = vim_commands(&mut events, 0.0, None);
            assert_eq!(commands, vec![Command::PaneLeft, Command::PaneRight]);
            assert!(events.is_empty());
        }
    }

    #[test]
    fn gh_opens_home_without_changing_gg_or_control_h() {
        use super::super::navigation::Command;
        let (commands, prefix) = vim_commands(
            &mut vec![
                event(Key::G, Modifiers::NONE, false),
                event(Key::H, Modifiers::NONE, false),
            ],
            1.0,
            None,
        );
        assert_eq!(commands, vec![Command::Home]);
        assert_eq!(prefix, None);
        let (commands, _) = vim_commands(
            &mut vec![event(Key::H, Modifiers::CTRL, false)],
            1.1,
            Some(1.0),
        );
        assert_eq!(commands, vec![Command::PaneLeft]);
        let (commands, _) = vim_commands(
            &mut vec![event(Key::H, Modifiers::NONE, false)],
            2.0,
            Some(1.0),
        );
        assert_eq!(
            commands,
            vec![Command::Left],
            "expired g must not open Home"
        );
    }

    #[test]
    fn question_mark_help_handles_key_and_text_layouts_without_stealing_slash() {
        for mut events in [
            vec![event(Key::Questionmark, Modifiers::NONE, false)],
            vec![event(Key::Questionmark, Modifiers::SHIFT, false)],
            vec![event(Key::Slash, Modifiers::SHIFT, false)],
            vec![egui::Event::Text("?".into())],
            vec![
                event(Key::Slash, Modifiers::NONE, false),
                egui::Event::Text("?".into()),
            ],
        ] {
            assert!(consume_shortcut_help(&mut events));
            assert!(events.is_empty());
        }
        let mut slash = vec![
            event(Key::Slash, Modifiers::NONE, false),
            egui::Event::Text("/".into()),
        ];
        assert!(!consume_shortcut_help(&mut slash));
        assert_eq!(slash.len(), 2);
    }

    #[test]
    fn shortcut_constants_name_the_platform_modifier() {
        let expected = if cfg!(target_os = "macos") {
            ["Cmd+B", "Cmd+Q", "Cmd+Shift+M", "Cmd+Shift+K"]
        } else {
            ["Ctrl+B", "Ctrl+Q", "Ctrl+M", "Ctrl+Shift+K"]
        };
        assert_eq!(
            [
                SIDEBAR_SHORTCUT,
                QUIT_SHORTCUT,
                WINAMP_SHORTCUT,
                MILKDROP_SHORTCUT,
            ],
            expected
        );
    }

    #[test]
    fn shortcut_dialog_never_names_the_other_command_modifier() {
        let other = if cfg!(target_os = "macos") {
            "Ctrl+"
        } else {
            "Cmd+"
        };
        for (keys, _) in SHORTCUTS {
            assert!(!keys.contains(other), "wrong modifier in {keys}");
        }
    }

    #[test]
    fn shortcut_dialog_names_platform_reserved_alternatives() {
        let label = |description| {
            SHORTCUTS
                .iter()
                .find(|(_, candidate)| *candidate == description)
                .map(|(keys, _)| *keys)
                .unwrap()
        };
        if cfg!(target_os = "macos") {
            assert_eq!(label("Home"), "Cmd+Shift+H");
            assert_eq!(label("Winamp mini player"), "Cmd+Shift+M");
        } else {
            assert_eq!(label("Home"), "Ctrl+H");
            assert_eq!(label("Winamp mini player"), "Ctrl+M");
        }
    }

    #[test]
    fn b_toggles_the_playing_song_in_liked_songs() {
        let root = std::env::temp_dir().join(format!(
            "fastpotify-like-shortcut-test-{}",
            std::process::id()
        ));
        let dirs = AppDirs {
            config: root.join("config"),
            state: root.join("state"),
            cache: root.join("cache"),
        };
        let mut app = App::new(
            &crate::backend::Waker::default(),
            dirs,
            Settings::default(),
            AppOptions {
                media_controls: false,
                restore_sign_in: false,
                tray: false,
            },
        );
        crate::demo::populate(&mut app);

        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: Key::B,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |_ui| handle(&mut app, &ctx));
        output.textures_delta.clear();

        assert!(matches!(
            app.actions.as_slice(),
            [Action::ToggleSaved(uri)] if uri == "spotify:track:trk0"
        ));
        app.backend.shutdown();
        let _ = std::fs::remove_dir_all(root);
    }
}
