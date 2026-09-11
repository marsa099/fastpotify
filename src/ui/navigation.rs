//! Shared, non-modal list navigation. Views resolve keys against display order
//! and emit state changes; App applies them after drawing.

use crate::app::App;
use crate::model::{Action, Page};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    Sidebar,
    #[default]
    Main,
    Queue,
}

impl Pane {
    pub fn adjacent(self, right: bool, sidebar: bool, queue: bool) -> Self {
        let panes = [Self::Sidebar, Self::Main, Self::Queue];
        let visible: Vec<_> = panes
            .into_iter()
            .filter(|pane| match pane {
                Self::Sidebar => sidebar,
                Self::Main => true,
                Self::Queue => queue,
            })
            .collect();
        let current = visible
            .iter()
            .position(|pane| *pane == self)
            .unwrap_or_else(|| visible.iter().position(|pane| *pane == Self::Main).unwrap());
        visible[if right {
            (current + 1).min(visible.len() - 1)
        } else {
            current.saturating_sub(1)
        }]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Up,
    Down,
    First,
    Last,
    HalfUp,
    HalfDown,
    Play,
    Open,
    Clear,
    Left,
    Right,
    PaneLeft,
    PaneRight,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub owner: Option<egui::Id>,
    pub row: Option<usize>,
    pub key: Option<egui::Id>,
    /// Follow appended pages until the real end is loaded.
    pub end: bool,
    /// Wait for the first page after gg from a playlist position window.
    pub start: bool,
}

#[derive(Default)]
pub struct Navigation {
    pub pane: Pane,
    pub cursors: [Cursor; 3],
    pub g_at: Option<f64>,
}

impl Navigation {
    pub fn active_pane(&self, sidebar: bool, queue: bool) -> Pane {
        match self.pane {
            Pane::Sidebar if !sidebar => Pane::Main,
            Pane::Queue if !queue => Pane::Main,
            pane => pane,
        }
    }

    pub fn cursor(&self, pane: Pane) -> Cursor {
        self.cursors[pane as usize]
    }
}

pub fn outline(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        egui::CornerRadius::same(6),
        egui::Stroke::new(2.0, ui.visuals().weak_text_color()),
        egui::StrokeKind::Inside,
    );
}

pub fn pane_move(app: &App, command: Command, pane: Pane) -> Option<bool> {
    match command {
        Command::PaneLeft => Some(false),
        Command::PaneRight => Some(true),
        Command::Left | Command::Right
            if pane != Pane::Main || !app.grid_navigation.active_on(app.page()) =>
        {
            Some(command == Command::Right)
        }
        _ => None,
    }
}

pub fn moved(row: Option<usize>, len: usize, command: Command, half: usize) -> Option<usize> {
    if len == 0 || command == Command::Clear {
        return None;
    }
    let last = len - 1;
    let half = half.max(1);
    Some(match command {
        Command::First => 0,
        Command::Last => last,
        Command::Down => row.map_or(0, |row| row.saturating_add(1).min(last)),
        Command::Up => row.map_or(last, |row| row.saturating_sub(1).min(last)),
        Command::HalfDown => row.unwrap_or(0).saturating_add(half).min(last),
        Command::HalfUp => row.unwrap_or(last).saturating_sub(half).min(last),
        _ => return row.filter(|row| *row < len),
    })
}

pub struct View {
    pub cursor: Cursor,
    pub command: Option<Command>,
    pub changed: bool,
    pub scroll: bool,
    pub focused: bool,
}

impl View {
    /// Scroll even when the destination row has not been virtualized yet.
    pub fn scroll_rows(&self, ui: &mut egui::Ui, offset: usize, len: usize, height: f32) {
        if self.scroll
            && let Some(row) = self.cursor.row
            && row >= offset
            && row - offset < len
        {
            let top = ui.cursor().top() + (row - offset) as f32 * height;
            ui.scroll_to_rect(
                egui::Rect::from_min_size(
                    egui::pos2(ui.max_rect().left(), top),
                    egui::vec2(ui.available_width(), height),
                ),
                None,
            );
        }
    }

    pub fn finish_boundaries(&self, app: &mut App, pane: Pane, end: bool, start: bool) {
        let cursor = Cursor {
            end: self.cursor.end && !end,
            start: self.cursor.start && !start,
            ..self.cursor
        };
        if cursor != self.cursor {
            app.actions.push(Action::NavigationCursor { pane, cursor });
        }
    }

    pub fn picked(&self, row: usize) -> bool {
        self.focused && self.cursor.row == Some(row)
    }
}

/// `owner` identifies sorting/filtering, while `key_at` detects replacement or
/// reordering without clearing the cursor when lazy loading only appends rows.
pub fn view(
    app: &mut App,
    ui: &egui::Ui,
    pane: Pane,
    owner: egui::Id,
    len: usize,
    height: f32,
    key_at: impl Fn(usize) -> egui::Id,
) -> View {
    if !app.settings.vim_keys {
        return View {
            cursor: Cursor::default(),
            command: None,
            changed: false,
            scroll: false,
            focused: false,
        };
    }
    if pane == Pane::Main {
        app.actions.push(Action::GridRows);
    }
    let grid_active = pane == Pane::Main && app.grid_navigation.active_on(app.page());
    let old = app.navigation.cursor(pane);
    // Route in event order: h followed by j within one frame must move a
    // sidebar row, not the main list that was focused at frame start.
    let mut active = app
        .navigation
        .active_pane(app.settings.sidebar_visible, app.show_queue_panel);
    let mut commands = Vec::new();
    for action in &app.actions {
        if let Action::Navigate(command) = action {
            if active == pane && !grid_active {
                commands.push(*command);
            }
            if let Some(right) = pane_move(app, *command, active) {
                active = active.adjacent(right, app.settings.sidebar_visible, app.show_queue_panel);
            }
        }
    }
    let focused = active == pane && !grid_active;
    if focused {
        outline(ui, ui.clip_rect());
    }
    let mut cursor = if old.owner == Some(owner) {
        old
    } else {
        Cursor {
            owner: Some(owner),
            ..Default::default()
        }
    };
    if cursor
        .row
        .is_some_and(|row| row >= len || Some(key_at(row)) != cursor.key)
    {
        cursor.row = None;
        cursor.key = None;
    }
    let half = (ui.clip_rect().height() / height / 2.0).floor().max(1.0) as usize;
    let mut command = None;
    for next in commands {
        cursor.row = moved(cursor.row, len, next, half);
        cursor.end = next == Command::Last;
        cursor.start = next == Command::First;
        command = Some(next);
    }
    if focused && cursor.end {
        cursor.row = len.checked_sub(1);
    } else if focused && cursor.start {
        cursor.row = (len > 0).then_some(0);
    }
    if focused
        && pane == Pane::Main
        && command == Some(Command::Down)
        && old.row == len.checked_sub(1)
        && app.grid_navigation.page.as_ref() == Some(app.page())
        && !app.grid_navigation.cards.is_empty()
    {
        app.actions.push(Action::FocusGrid);
    }
    cursor.key = cursor.row.map(key_at);
    let changed = cursor != old;
    if app.settings.vim_keys && changed {
        app.actions.push(Action::NavigationCursor { pane, cursor });
    }
    View {
        cursor,
        command,
        changed,
        scroll: focused
            && (changed
                || command.is_some_and(|command| {
                    !matches!(command, Command::Play | Command::Open | Command::Clear)
                })),
        focused,
    }
}

pub fn pick(app: &mut App, pane: Pane, owner: egui::Id, row: usize, key: egui::Id) {
    if app.settings.vim_keys {
        app.actions.push(Action::NavigationFocus(pane));
        app.actions.push(Action::NavigationCursor {
            pane,
            cursor: Cursor {
                owner: Some(owner),
                row: Some(row),
                key: Some(key),
                end: false,
                start: false,
            },
        });
    }
}

/// Small, non-virtualized track sections (search and artist Popular).
/// Their row stride includes the surrounding Ui's item spacing.
pub fn playable_view(
    app: &mut App,
    ui: &mut egui::Ui,
    owner: egui::Id,
    items: &[crate::api::models::PlayableItem],
    context: &crate::model::RowContext,
    height: f32,
) -> View {
    let navigation = view(app, ui, Pane::Main, owner, items.len(), height, |row| {
        egui::Id::new(items[row].uri())
    });
    navigation.scroll_rows(ui, 0, items.len(), height);
    navigation.finish_boundaries(app, Pane::Main, true, true);
    if let Some(row) = navigation.cursor.row {
        let item = &items[row];
        let action = match navigation.command {
            Some(Command::Play) if can_play(item) => Some(Action::PlayFromRow {
                context: context.clone(),
                uri: item.uri().to_string(),
                index: row as u32,
            }),
            Some(Command::Open) => album(item),
            _ => None,
        };
        if let Some(action) = action {
            app.actions.push(action);
        }
    }
    navigation
}

pub fn can_play(item: &crate::api::models::PlayableItem) -> bool {
    match item {
        crate::api::models::PlayableItem::Track(track) => {
            track.is_playable != Some(false) && !track.is_local
        }
        crate::api::models::PlayableItem::Episode(_) => true,
    }
}

pub fn album(item: &crate::api::models::PlayableItem) -> Option<Action> {
    match item {
        crate::api::models::PlayableItem::Track(track) => track
            .album
            .as_ref()
            .map(|album| Action::Open(Page::Album(album.id.clone()))),
        crate::api::models::PlayableItem::Episode(episode) => episode
            .show
            .as_ref()
            .map(|show| Action::Open(Page::Show(show.id.clone()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_app(test: impl FnOnce(&mut App, &egui::Context)) {
        use crate::{app::AppOptions, paths::AppDirs, settings::Settings};
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "fastpotify-vim-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let ctx = egui::Context::default();
        let mut app = App::new(
            &crate::backend::Waker::default(),
            AppDirs {
                config: root.join("config"),
                state: root.join("state"),
                cache: root.join("cache"),
            },
            Settings {
                vim_keys: true,
                ..Default::default()
            },
            AppOptions {
                media_controls: false,
                restore_sign_in: false,
                tray: false,
            },
        );
        app.attach(&ctx);
        crate::demo::populate(&mut app);
        test(&mut app, &ctx);
        app.backend.shutdown();
        let _ = std::fs::remove_dir_all(root);
    }

    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<Action> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| crate::ui::show(app, ui),
        );
        output.textures_delta.clear();
        let actions = std::mem::take(&mut app.actions);
        app.grid_navigation.prepare(&actions);
        for action in &actions {
            app.apply(action.clone(), ctx);
        }
        // Demo actions can enqueue follow-up actions; do not carry them into
        // the next frame's keyboard request collection.
        while !app.actions.is_empty() {
            for action in std::mem::take(&mut app.actions) {
                app.apply(action, ctx);
            }
        }
        actions
    }

    fn press(
        app: &mut App,
        ctx: &egui::Context,
        key: egui::Key,
        modifiers: egui::Modifiers,
    ) -> Vec<Action> {
        frame(
            app,
            ctx,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
        )
    }

    #[test]
    fn vim_demo_navigates_all_panes_and_emits_queue_skip_actions() {
        with_app(|app, ctx| {
            app.open(Page::Playlist("pl1".into()));
            app.show_queue_panel = true;
            frame(app, ctx, vec![]);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert_eq!(app.navigation.cursor(Pane::Main).row, Some(0));
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert_eq!(app.navigation.cursor(Pane::Main).row, Some(1));
            assert_eq!(app.selection.as_ref().unwrap().2.anchor, Some(1));
            let actions = press(app, ctx, egui::Key::Enter, egui::Modifiers::NONE);
            assert!(
                actions
                    .iter()
                    .any(|action| matches!(action, Action::PlayFromRow { index: 1, .. }))
            );
            press(app, ctx, egui::Key::H, egui::Modifiers::NONE);
            assert_eq!(app.navigation.pane, Pane::Sidebar);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert_eq!(app.navigation.cursor(Pane::Sidebar).row, Some(0));
            let actions = press(app, ctx, egui::Key::Enter, egui::Modifiers::NONE);
            assert!(
                actions
                    .iter()
                    .any(|action| matches!(action, Action::Open(_)))
            );
            press(app, ctx, egui::Key::L, egui::Modifiers::NONE);
            press(app, ctx, egui::Key::L, egui::Modifiers::NONE);
            assert_eq!(app.navigation.pane, Pane::Queue);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert_eq!(app.navigation.cursor(Pane::Queue).row, Some(1));
            let actions = press(app, ctx, egui::Key::Enter, egui::Modifiers::NONE);
            assert!(actions.iter().any(|action| matches!(
                action,
                Action::PlayFromRow {
                    context: crate::model::RowContext::Queue,
                    index: 1,
                    ..
                }
            )));
            press(app, ctx, egui::Key::Escape, egui::Modifiers::NONE);
            assert_eq!(app.navigation.cursor(Pane::Queue).row, None);
        });
    }

    #[test]
    fn vim_is_disabled_by_default_and_blocked_by_text_focus_and_dialogs() {
        with_app(|app, ctx| {
            app.open(Page::Playlist("pl1".into()));
            app.settings.vim_keys = false;
            let actions = press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert!(
                !actions
                    .iter()
                    .any(|action| matches!(action, Action::Navigate(_)))
            );
            app.settings.vim_keys = true;
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("typing-test")));
            let actions = press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert!(
                !actions
                    .iter()
                    .any(|action| matches!(action, Action::Navigate(_)))
            );
            ctx.memory_mut(|memory| memory.surrender_focus(egui::Id::new("typing-test")));
            app.dialog = Some(crate::model::Dialog::Shortcuts);
            let actions = press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert!(
                !actions
                    .iter()
                    .any(|action| matches!(action, Action::Navigate(_)))
            );
            press(app, ctx, egui::Key::Escape, egui::Modifiers::NONE);
            assert!(app.dialog.is_none());
        });
    }

    #[test]
    fn vim_virtual_destination_scrolls_into_view() {
        with_app(|app, ctx| {
            let owner = egui::Id::new("virtual-navigation-test");
            let mut drawn = Vec::new();
            for pass in 0..8 {
                if pass == 1 {
                    app.actions.push(Action::Navigate(Command::Last));
                }
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(400.0, 300.0),
                        )),
                        time: Some(pass as f64),
                        ..Default::default()
                    },
                    |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let navigation = view(app, ui, Pane::Main, owner, 100, 40.0, |row| {
                                egui::Id::new(row)
                            });
                            navigation.scroll_rows(ui, 0, 100, 40.0);
                            drawn.clear();
                            super::super::widgets::virtual_rows(ui, 100, 40.0, |ui, row| {
                                drawn.push(row);
                                ui.allocate_space(egui::vec2(ui.available_width(), 40.0));
                            });
                        });
                    },
                );
                output.textures_delta.clear();
                for action in std::mem::take(&mut app.actions) {
                    app.apply(action, ctx);
                }
            }
            assert_eq!(app.navigation.cursor(Pane::Main).row, Some(99));
            assert!(
                drawn.contains(&99),
                "the offscreen destination must be drawn after scrolling: {drawn:?}"
            );
            assert!(drawn.len() < 15, "navigation must preserve virtualization");
        });
    }

    #[test]
    fn vim_search_and_artist_tracks_play_and_open_the_selected_album() {
        with_app(|app, ctx| {
            for page in [Page::Search, Page::Artist("art0".into())] {
                app.open(page);
                frame(app, ctx, vec![]);
                press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
                assert_eq!(app.navigation.cursor(Pane::Main).row, Some(0));
                let actions = press(app, ctx, egui::Key::Enter, egui::Modifiers::NONE);
                assert!(
                    actions
                        .iter()
                        .any(|action| matches!(action, Action::PlayFromRow { index: 0, .. }))
                );
                let actions = press(app, ctx, egui::Key::O, egui::Modifiers::NONE);
                assert!(
                    actions
                        .iter()
                        .any(|action| matches!(action, Action::Open(Page::Album(_))))
                );
            }
        });
    }

    #[test]
    fn vim_collection_uses_sorted_and_filtered_display_order() {
        with_app(|app, ctx| {
            let page = Page::Playlist("pl1".into());
            app.open(page.clone());
            frame(app, ctx, vec![]);
            let items = app.table_rows[&page].items.clone();
            let expected = items
                .iter()
                .min_by_key(|(item, _, _)| item.name().to_lowercase())
                .unwrap()
                .0
                .uri()
                .to_string();
            app.table_sorts.insert(
                page.clone(),
                crate::model::TableSort {
                    column: crate::model::SortColumn::Title,
                    ascending: true,
                },
            );
            frame(app, ctx, vec![]);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            let actions = press(app, ctx, egui::Key::Enter, egui::Modifiers::NONE);
            assert!(actions.iter().any(|action| matches!(action, Action::PlayFromRow { context: crate::model::RowContext::View { uris, .. }, uri, index: 0 } if *uri == expected && uris[0] == expected)));
            app.table_sorts.remove(&page);
            let (index, (item, _, _)) = items.iter().enumerate().next_back().unwrap();
            let title = item.name().to_string();
            app.playlist_pages.get_mut("pl1").unwrap().filter = title.clone();
            frame(app, ctx, vec![]);
            assert_eq!(app.navigation.cursor(Pane::Main).row, None);
            press(app, ctx, egui::Key::G, egui::Modifiers::SHIFT);
            let actions = press(app, ctx, egui::Key::Enter, egui::Modifiers::NONE);
            assert!(actions.iter().any(|action| matches!(action, Action::PlayFromRow { uri, index: actual, .. } if uri == item.uri() && *actual == index as u32)));
        });
    }

    #[test]
    fn vim_queue_changes_clear_duplicate_positional_selection() {
        with_app(|app, ctx| {
            app.show_queue_panel = true;
            app.navigation.pane = Pane::Queue;
            let queue = match &mut app.queue {
                crate::model::Loadable::Loaded(queue) => queue,
                _ => panic!("demo queue must be loaded"),
            };
            let item = queue.queue[0].clone();
            queue.queue = vec![item.clone(), item.clone(), item];
            frame(app, ctx, vec![]);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert_eq!(app.navigation.cursor(Pane::Queue).row, Some(1));
            if let crate::model::Loadable::Loaded(queue) = &mut app.queue {
                queue.queue.remove(0);
            }
            frame(app, ctx, vec![]);
            assert_eq!(app.navigation.cursor(Pane::Queue).row, None);
            let actions = press(app, ctx, egui::Key::Enter, egui::Modifiers::NONE);
            assert!(
                !actions
                    .iter()
                    .any(|action| matches!(action, Action::PlayFromRow { .. }))
            );
        });
    }

    #[test]
    fn vim_end_follows_appended_rows_but_reordering_clears_selection() {
        with_app(|app, ctx| {
            let owner = egui::Id::new("lazy-list");
            let mut draw = |keys: &[usize], command: Option<Command>| {
                if let Some(command) = command {
                    app.actions.push(Action::Navigate(command));
                }
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    view(app, ui, Pane::Main, owner, keys.len(), 40.0, |row| {
                        egui::Id::new(keys[row])
                    });
                });
                output.textures_delta.clear();
                for action in std::mem::take(&mut app.actions) {
                    app.apply(action, ctx);
                }
                app.navigation.cursor(Pane::Main)
            };
            assert_eq!(draw(&[0, 1], Some(Command::Last)).row, Some(1));
            assert_eq!(draw(&[0, 1, 2, 3], None).row, Some(3));
            let cursor = draw(&[0, 1, 2, 3], Some(Command::Up));
            assert_eq!(cursor.row, Some(2));
            assert!(!cursor.end);
            assert_eq!(draw(&[0, 1, 9, 3], None).row, None);
            assert_eq!(draw(&[], Some(Command::Last)).row, None);
            assert_eq!(draw(&[0, 1], None).row, Some(1));
        });
    }

    #[test]
    fn vim_lyrics_keeps_its_old_binding_when_disabled() {
        with_app(|app, ctx| {
            app.settings.vim_keys = false;
            let actions = press(app, ctx, egui::Key::L, egui::Modifiers::NONE);
            assert!(
                actions
                    .iter()
                    .any(|action| matches!(action, Action::ToggleLyricsPanel))
            );
            app.settings.vim_keys = true;
            let actions = press(app, ctx, egui::Key::L, egui::Modifiers::NONE);
            assert!(
                !actions
                    .iter()
                    .any(|action| matches!(action, Action::ToggleLyricsPanel))
            );
            let actions = press(app, ctx, egui::Key::L, egui::Modifiers::SHIFT);
            assert!(
                actions
                    .iter()
                    .any(|action| matches!(action, Action::ToggleLyricsPanel))
            );
        });
    }

    #[test]
    fn vim_routes_pane_switch_and_row_movement_in_the_same_frame() {
        with_app(|app, ctx| {
            app.open(Page::Playlist("pl1".into()));
            frame(app, ctx, vec![]);
            frame(
                app,
                ctx,
                [egui::Key::H, egui::Key::J]
                    .into_iter()
                    .map(|key| egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    })
                    .collect(),
            );
            assert_eq!(app.navigation.pane, Pane::Sidebar);
            assert_eq!(app.navigation.cursor(Pane::Sidebar).row, Some(0));
            assert_eq!(app.navigation.cursor(Pane::Main).row, None);
        });
    }

    #[cfg(feature = "demo")]
    #[test]
    fn vim_demo_flag_enables_navigation() {
        with_app(|app, ctx| {
            app.settings.vim_keys = false;
            crate::demo::apply_flags(app, Some("playlist:pl1"), Some("queue,vim-keys"));
            assert!(app.settings.vim_keys);
            frame(app, ctx, vec![]);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert_eq!(app.navigation.cursor(Pane::Main).row, Some(0));
        });
    }

    #[test]
    fn vim_selection_survives_view_refresh() {
        with_app(|app, ctx| {
            app.open(Page::Playlist("pl1".into()));
            frame(app, ctx, vec![]);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert_eq!(app.navigation.cursor(Pane::Main).row, Some(0));
            // Appending a page clears the mouse selection's count-based view
            // token. The still-valid keyboard cursor must remain visible.
            app.selection = None;
            frame(app, ctx, vec![]);
            assert_eq!(app.selection.as_ref().unwrap().2.anchor, Some(0));
            let owner = app.navigation.cursor(Pane::Main).owner.unwrap();
            let key = app.navigation.cursor(Pane::Main).key.unwrap();
            let view = app.selection.as_ref().unwrap().1.clone();
            app.apply(
                Action::PickTableRow {
                    page: app.page().clone(),
                    view,
                    row: 0,
                    pick: crate::model::RowPick::Only,
                    len: 100,
                    owner,
                    key,
                },
                ctx,
            );
            frame(app, ctx, vec![]);
            assert!(
                app.selection.is_none(),
                "clicking the sole selected row still clears it"
            );
            assert_eq!(app.navigation.cursor(Pane::Main).row, None);
        });
    }

    #[test]
    fn vim_last_waits_for_inflight_collection_and_sidebar_pages() {
        with_app(|app, ctx| {
            app.open(Page::Playlist("pl1".into()));
            let page = app.playlist_pages.get_mut("pl1").unwrap();
            page.items.next_offset = Some(page.items.items.len() as u32);
            frame(app, ctx, vec![]);
            press(app, ctx, egui::Key::G, egui::Modifiers::SHIFT);
            assert!(app.playlist_pages["pl1"].items.loading);
            frame(app, ctx, vec![]);
            assert!(
                app.navigation.cursor(Pane::Main).end,
                "inflight is not complete"
            );
            let page = app.playlist_pages.get_mut("pl1").unwrap();
            page.items
                .items
                .push(page.items.items.last().unwrap().clone());
            page.items.loading = false;
            page.items.next_offset = None;
            page.items.revision += 1;
            let last = page.items.items.len() - 1;
            frame(app, ctx, vec![]);
            assert_eq!(app.navigation.cursor(Pane::Main).row, Some(last));
            assert!(!app.navigation.cursor(Pane::Main).end);

            ctx.data_mut(|data| {
                data.insert_temp(
                    egui::Id::new("sidebar-filter"),
                    crate::settings::LibraryShelf::Albums,
                )
            });
            app.navigation.pane = Pane::Sidebar;
            app.library.albums.next_offset = Some(app.library.albums.items.len() as u32);
            frame(app, ctx, vec![]);
            press(app, ctx, egui::Key::G, egui::Modifiers::SHIFT);
            assert!(app.library.albums.loading);
            frame(app, ctx, vec![]);
            assert!(app.navigation.cursor(Pane::Sidebar).end);
            app.library.albums.loading = false;
            app.library.albums.next_offset = None;
            frame(app, ctx, vec![]);
            assert!(!app.navigation.cursor(Pane::Sidebar).end);
        });
    }

    #[test]
    fn vim_first_returns_from_a_playlist_position_window() {
        with_app(|app, ctx| {
            app.open(Page::Playlist("pl1".into()));
            let page = app.playlist_pages.get_mut("pl1").unwrap();
            let items = page.items.items.clone();
            page.items.base_offset = 100;
            page.items.total = Some(140);
            page.items.revision += 1;
            frame(app, ctx, vec![]);
            app.actions.push(Action::Navigate(Command::First));
            let actions = frame(app, ctx, vec![]);
            assert!(actions.iter().any(|action| matches!(
                action,
                Action::JumpToPlaylistPosition { position: 1, .. }
            )));
            assert_eq!(app.playlist_pages["pl1"].items.base_offset, 0);
            frame(app, ctx, vec![]);
            assert!(app.navigation.cursor(Pane::Main).start);
            let page = app.playlist_pages.get_mut("pl1").unwrap();
            page.items.items = items;
            page.items.loading = false;
            page.items.revision += 1;
            frame(app, ctx, vec![]);
            assert_eq!(app.navigation.cursor(Pane::Main).row, Some(0));
            assert!(!app.navigation.cursor(Pane::Main).start);
        });
    }

    #[test]
    fn vim_disabled_queue_body_click_does_not_require_a_keyboard_cursor() {
        with_app(|app, ctx| {
            app.settings.vim_keys = false;
            let mut position = egui::Pos2::ZERO;
            for pass in 0..4 {
                let events = if pass < 2 {
                    vec![]
                } else {
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed: pass == 2,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]
                };
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(500.0, 700.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| crate::ui::queue::page(app, ui),
                );
                output.textures_delta.clear();
                fn find(shape: &egui::epaint::Shape) -> Option<egui::Pos2> {
                    match shape {
                        egui::epaint::Shape::Text(text) if text.galley.job.text == "Otomo" => {
                            Some(text.visual_bounding_rect().center())
                        }
                        egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(find),
                        _ => None,
                    }
                }
                position = output
                    .shapes
                    .iter()
                    .find_map(|shape| find(&shape.shape))
                    .expect("the first queue row must be visible");
            }
            assert!(
                app.actions.is_empty(),
                "a queue row-body click with Vim disabled emits no playback or navigation actions"
            );
        });
    }

    #[test]
    fn grid_hjkl_moves_spatially_and_enter_opens_in_the_content_pane() {
        with_app(|app, ctx| {
            app.open(Page::Home);
            app.show_queue_panel = true;
            frame(app, ctx, vec![]);
            frame(app, ctx, vec![]);
            assert!(app.grid_navigation.active_on(&Page::Home));
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            let selected = |app: &App| {
                app.grid_navigation
                    .cards
                    .iter()
                    .find(|card| Some(card.id) == app.grid_navigation.selected)
                    .unwrap()
                    .clone()
            };
            let first = selected(app);
            press(app, ctx, egui::Key::L, egui::Modifiers::NONE);
            let right = selected(app);
            assert!(right.rect.center().x > first.rect.center().x);
            assert_eq!(app.page(), &Page::Home, "l moves, never opens a card");
            assert_eq!(app.navigation.pane, Pane::Main);
            press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            assert!(selected(app).rect.center().y > right.rect.center().y);
            press(app, ctx, egui::Key::K, egui::Modifiers::NONE);
            assert_eq!(selected(app).id, right.id);
            press(app, ctx, egui::Key::H, egui::Modifiers::NONE);
            assert_eq!(selected(app).id, first.id);
            press(app, ctx, egui::Key::L, egui::Modifiers::NONE);
            let destination = selected(app).page;
            press(app, ctx, egui::Key::Enter, egui::Modifiers::NONE);
            assert_eq!(app.page(), &destination);
            assert_eq!(app.navigation.pane, Pane::Main);
            press(app, ctx, egui::Key::L, egui::Modifiers::CTRL);
            assert_eq!(app.navigation.pane, Pane::Queue);
            press(app, ctx, egui::Key::H, egui::Modifiers::CTRL);
            assert_eq!(app.navigation.pane, Pane::Main);
            press(app, ctx, egui::Key::H, egui::Modifiers::CTRL);
            assert_eq!(app.navigation.pane, Pane::Sidebar);
        });
    }

    #[test]
    fn grid_virtual_cards_keep_offscreen_targets_and_scroll_to_the_last_card() {
        with_app(|app, ctx| {
            let template = app.library.albums.items[0].clone();
            app.library.albums.items = (0..180)
                .map(|index| {
                    let mut saved = template.clone();
                    saved.album.id = format!("grid-album-{index}");
                    saved.album.uri = format!("spotify:album:grid-album-{index}");
                    saved.album.name = format!("Grid album {index}");
                    saved
                })
                .collect();
            app.library.albums.next_offset = None;
            app.open(Page::Albums);
            for _ in 0..3 {
                frame(app, ctx, vec![]);
            }
            assert_eq!(
                app.grid_navigation.cards.len(),
                180,
                "rendered and virtual IDs must agree"
            );
            assert!(
                app.grid_navigation
                    .cards
                    .iter()
                    .filter(|card| card.response.is_some())
                    .count()
                    < 40
            );
            press(app, ctx, egui::Key::G, egui::Modifiers::SHIFT);
            for _ in 0..30 {
                frame(app, ctx, vec![]);
            }
            let card = app
                .grid_navigation
                .cards
                .iter()
                .find(|card| Some(card.id) == app.grid_navigation.selected)
                .unwrap();
            assert_eq!(card.page, Page::Album("grid-album-179".into()));
            assert!(
                card.response.is_some(),
                "selected offscreen card must become rendered"
            );
            assert!(
                app.grid_navigation
                    .cards
                    .iter()
                    .filter(|card| card.response.is_some())
                    .count()
                    < 40
            );
            press(app, ctx, egui::Key::O, egui::Modifiers::NONE);
            assert_eq!(app.page(), &Page::Album("grid-album-179".into()));
        });
    }

    #[test]
    fn grid_library_search_and_artist_cards_register_openable_targets() {
        with_app(|app, ctx| {
            for page in [Page::Albums, Page::Artists, Page::Podcasts] {
                app.open(page.clone());
                for _ in 0..2 {
                    frame(app, ctx, vec![]);
                }
                assert!(app.grid_navigation.active_on(&page));
                assert!(!app.grid_navigation.cards.is_empty());
            }
            for filter in [
                crate::model::SearchFilter::Albums,
                crate::model::SearchFilter::Artists,
                crate::model::SearchFilter::Playlists,
                crate::model::SearchFilter::Podcasts,
            ] {
                app.open(Page::Search);
                app.search.filter = filter;
                for _ in 0..2 {
                    frame(app, ctx, vec![]);
                }
                assert!(!app.grid_navigation.cards.is_empty());
                press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
                let destination = app
                    .grid_navigation
                    .cards
                    .iter()
                    .find(|card| Some(card.id) == app.grid_navigation.selected)
                    .unwrap()
                    .page
                    .clone();
                press(app, ctx, egui::Key::O, egui::Modifiers::NONE);
                assert_eq!(app.page(), &destination);
            }
            app.open(Page::Artist("art0".into()));
            for _ in 0..2 {
                frame(app, ctx, vec![]);
            }
            assert!(!app.grid_navigation.active);
            for _ in 0..6 {
                press(app, ctx, egui::Key::J, egui::Modifiers::NONE);
            }
            assert!(
                app.grid_navigation.active,
                "j below Popular enters the card grid"
            );
            assert!(app.grid_navigation.selected.is_some());
            press(app, ctx, egui::Key::K, egui::Modifiers::NONE);
            assert!(
                !app.grid_navigation.active,
                "k above the grid returns to Popular"
            );
        });
    }

    #[test]
    fn grid_control_h_l_are_left_untouched_in_inputs() {
        with_app(|app, ctx| {
            for key in [egui::Key::H, egui::Key::L] {
                let input_id = egui::Id::new("search-input-test");
                ctx.memory_mut(|memory| memory.request_focus(input_id));
                app.actions.clear();
                let mut output = ctx.run_ui(egui::RawInput {
                    events: vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::CTRL }],
                    ..Default::default()
                }, |_ui| {
                    super::super::keys::handle(app, ctx);
                    assert!(ctx.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Key { key: actual, pressed: true, .. } if *actual == key))));
                });
                output.textures_delta.clear();
                assert!(
                    !app.actions
                        .iter()
                        .any(|action| matches!(action, Action::Navigate(_) | Action::Open(_)))
                );
            }
        });
    }

    #[test]
    fn movement_clamps_and_handles_empty_lists() {
        for command in [
            Command::Up,
            Command::Down,
            Command::First,
            Command::Last,
            Command::HalfUp,
            Command::HalfDown,
        ] {
            assert_eq!(moved(None, 0, command, 10), None);
        }
        assert_eq!(moved(None, 20, Command::Down, 4), Some(0));
        assert_eq!(moved(None, 20, Command::Up, 4), Some(19));
        assert_eq!(moved(Some(0), 20, Command::Up, 4), Some(0));
        assert_eq!(moved(Some(19), 20, Command::Down, 4), Some(19));
        assert_eq!(moved(Some(7), 20, Command::HalfDown, 4), Some(11));
        assert_eq!(moved(Some(7), 20, Command::HalfUp, 4), Some(3));
        assert_eq!(moved(Some(7), 20, Command::Clear, 4), None);
    }

    #[test]
    fn pane_movement_skips_hidden_panels_and_does_not_wrap() {
        assert_eq!(Pane::Main.adjacent(false, false, true), Pane::Main);
        assert_eq!(Pane::Main.adjacent(true, true, false), Pane::Main);
        assert_eq!(Pane::Main.adjacent(false, true, true), Pane::Sidebar);
        assert_eq!(Pane::Sidebar.adjacent(true, true, true), Pane::Main);
        assert_eq!(Pane::Main.adjacent(true, true, true), Pane::Queue);
        assert_eq!(Pane::Queue.adjacent(true, true, true), Pane::Queue);
    }
}
