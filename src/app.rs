// SPDX-License-Identifier: MPL-2.0

use crate::config::Config;
use crate::fl;
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::dialog::file_chooser::{self, FileFilter};
use cosmic::iced::futures::{SinkExt, Stream};
use cosmic::iced::{Alignment, Font, Length, Subscription, keyboard};
use cosmic::prelude::*;
use cosmic::widget::menu::action::MenuAction as _;
use cosmic::widget::{self, about::About, markdown, menu, text_editor};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");
// Pop Icons (System76), CC-BY-SA-4.0 — see resources/icons/bundled/COPYING.
const ICON_PREVIEW: &[u8] = include_bytes!("../resources/icons/bundled/show-symbolic.svg");
const ICON_EDIT: &[u8] = include_bytes!("../resources/icons/bundled/edit-symbolic.svg");

/// Which representation of the document is currently shown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// Raw Markdown source, edited in a text editor.
    Source,
    /// Read-only rendered output.
    View,
}

/// An action deferred until the user resolves unsaved changes.
#[derive(Clone, Copy, Debug)]
pub enum PendingAction {
    /// Start a new, empty document.
    New,
    /// Prompt for a file to open.
    Open,
    /// Close the application.
    Quit,
}

/// A modal dialog shown over the application.
pub enum Dialog {
    /// Confirm discarding unsaved changes before performing the pending action.
    ConfirmDiscard(PendingAction),
    /// The open file changed on disk while there are unsaved local edits;
    /// holds the on-disk contents to load if the user chooses to.
    ConflictReload(String),
}

/// The line-ending convention of a document, preserved across save.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEnding {
    /// Unix line endings (`\n`). Default for new documents.
    Lf,
    /// Windows line endings (`\r\n`).
    Crlf,
}

impl LineEnding {
    /// Detects the line-ending convention used by some text.
    fn detect(text: &str) -> Self {
        if text.contains("\r\n") {
            Self::Crlf
        } else {
            Self::Lf
        }
    }

    /// Applies this convention to text whose newlines are normalized to `\n`.
    fn apply(self, text: String) -> String {
        match self {
            Self::Lf => text,
            // Normalize first so we never produce `\r\r\n`.
            Self::Crlf => text.replace("\r\n", "\n").replace('\n', "\r\n"),
        }
    }
}

/// Maximum number of undo snapshots retained.
const UNDO_LIMIT: usize = 256;

/// Base text size of the Source editor, in pixels (before zoom).
const EDITOR_TEXT_SIZE: f32 = 14.0;
/// Base body text size of the rendered View, in pixels (before zoom).
const VIEW_TEXT_SIZE: f32 = 16.0;
/// Pixels added/removed per zoom step.
const ZOOM_STEP: f32 = 2.0;
/// Zoom delta bounds, in pixels.
const ZOOM_MIN: f32 = -6.0;
const ZOOM_MAX: f32 = 32.0;

/// The kind of the current run of edits, used to coalesce undo steps.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum EditRun {
    #[default]
    None,
    Insert,
    Backspace,
}

/// A single search match: byte offsets within a line.
#[derive(Clone, Copy)]
struct Match {
    line: usize,
    start: usize,
    end: usize,
}

/// Find/replace state.
#[derive(Default)]
pub struct Search {
    /// Whether the find bar is shown.
    active: bool,
    /// Whether the replace row is shown.
    show_replace: bool,
    /// The current search query.
    query: String,
    /// The replacement text.
    replacement: String,
    /// Matches of `query` in the current buffer.
    matches: Vec<Match>,
    /// Index of the active match within `matches`.
    current: usize,
}

/// The currently open document.
pub struct Document {
    /// On-disk location, or `None` for a new unsaved document.
    path: Option<PathBuf>,
    /// The editable Markdown source buffer.
    content: text_editor::Content,
    /// Whether the buffer has unsaved changes.
    dirty: bool,
    /// The active view mode.
    mode: Mode,
    /// The line-ending convention to write back on save.
    line_ending: LineEnding,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            path: None,
            content: text_editor::Content::new(),
            dirty: false,
            mode: Mode::Source,
            line_ending: LineEnding::Lf,
        }
    }
}

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// Display a context drawer with the designated page if defined.
    context_page: ContextPage,
    /// The about page for this app.
    about: About,
    /// Key bindings for the application's menu bar.
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    /// Configuration data that persists between application runs.
    config: Config,
    /// The currently open document.
    document: Document,
    /// Parsed Markdown rendered in View mode; rebuilt from the source buffer.
    markdown: markdown::Content,
    /// A user-facing error message, shown as a dismissible banner.
    error: Option<String>,
    /// Find/replace state.
    search: Search,
    /// Buffer snapshots for undo (oldest first).
    undo_stack: Vec<String>,
    /// Buffer snapshots for redo.
    redo_stack: Vec<String>,
    /// The kind of the in-progress edit run, for coalescing undo steps.
    undo_run: EditRun,
    /// Zoom delta (pixels) added to the content text sizes.
    zoom: f32,
    /// The active modal dialog, if any.
    dialog: Option<Dialog>,
    /// An action to run once unsaved changes are resolved (e.g. after saving).
    pending: Option<PendingAction>,
    /// Set while the application is closing, to allow the window to close.
    quitting: bool,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    LaunchUrl(String),
    ToggleContextPage(ContextPage),
    UpdateConfig(Config),
    /// An edit action from the source text editor.
    Edit(text_editor::Action),
    /// Toggle between Source and View modes.
    ToggleMode,
    /// Show or hide the window header bar (decorations).
    ToggleHeaderBar,
    /// Start a new, empty document.
    New,
    /// Prompt for a file to open.
    OpenFile,
    /// A file was read from disk: its path and contents.
    FileOpened(PathBuf, String),
    /// Save the current document (prompting for a path if it has none).
    SaveFile,
    /// Save the current document under a newly chosen path.
    SaveFileAs,
    /// The document was written to disk at the given path.
    FileSaved(PathBuf),
    /// A file dialog was cancelled; no action needed.
    DialogCancelled,
    /// A file operation failed; show the message.
    DialogError(String),
    /// Dismiss the current error banner.
    DismissError,
    /// The window was asked to close while there are unsaved changes.
    RequestQuit,
    /// Confirm dialog: save, then continue the pending action.
    DialogSave,
    /// Confirm dialog: discard changes and continue the pending action.
    DialogDiscard,
    /// Confirm dialog: cancel the pending action.
    DialogCancel,
    /// The open file changed on disk (from the file watcher).
    FileChangedOnDisk,
    /// The on-disk contents were re-read after an external change.
    ExternalContent(Result<String, String>),
    /// Conflict dialog: keep the in-memory edits, ignore the disk version.
    KeepLocal,
    /// Conflict dialog: discard local edits and load the disk version.
    ReloadFromDisk,
    /// A keyboard event, matched against the application's key bindings.
    Key(keyboard::Event),
    /// Cut the editor selection to the clipboard.
    Cut,
    /// Copy the editor selection to the clipboard.
    Copy,
    /// Paste clipboard contents into the editor.
    Paste,
    /// Clipboard contents to paste (from an async read).
    Pasted(Option<String>),
    /// Select all text in the editor.
    SelectAll,
    /// Open the find bar.
    FindOpen,
    /// Open the find bar with the replace row.
    ReplaceOpen,
    /// Close the find/replace bar.
    FindClose,
    /// The find query changed.
    FindQueryChanged(String),
    /// The replacement text changed.
    ReplacementChanged(String),
    /// Move to the next match.
    FindNext,
    /// Move to the previous match.
    FindPrev,
    /// Replace the current match.
    ReplaceCurrent,
    /// Replace all matches.
    ReplaceAll,
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
    /// Increase the content text size.
    ZoomIn,
    /// Decrease the content text size.
    ZoomOut,
    /// Reset the content text size.
    ZoomReset,
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method: an optional
    /// file path to open on startup (e.g. from "Open with").
    type Flags = Option<PathBuf>;

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "dev.cosmic.CosmicMarkdown";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(
        core: cosmic::Core,
        flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Create the about widget
        let about = About::default()
            .name(fl!("app-title"))
            .icon(widget::icon::from_svg_bytes(APP_ICON))
            .version(env!("CARGO_PKG_VERSION"))
            .links([(fl!("repository"), REPOSITORY)])
            .license(env!("CARGO_PKG_LICENSE"));

        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            about,
            key_binds: key_binds(),
            // Optional configuration file for an application.
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| match Config::get_entry(&context) {
                    Ok(config) => config,
                    Err((_errors, config)) => config,
                })
                .unwrap_or_default(),
            document: Document::default(),
            markdown: markdown::Content::new(),
            error: None,
            search: Search::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_run: EditRun::None,
            zoom: 0.0,
            dialog: None,
            pending: None,
            quitting: false,
        };

        // If launched with a file path (e.g. via "Open with"), open it.
        if let Some(path) = flags {
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    app.document.mode = Mode::View;
                    app.load_contents(&contents);
                    app.document.path = Some(path);
                }
                Err(why) => {
                    app.error = Some(format!("failed to open {}: {why}", path.display()));
                }
            }
        }

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (app, command)
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        // Edit actions operate on the Source editor, so disable them in View mode.
        let source_mode = self.document.mode == Mode::Source;
        let edit_item = |label: String, action: MenuAction| {
            if source_mode {
                menu::Item::Button(label, None, action)
            } else {
                menu::Item::ButtonDisabled(label, None, action)
            }
        };
        // Undo/redo are additionally gated on history availability.
        let history_item = |label: String, action: MenuAction, enabled: bool| {
            if source_mode && enabled {
                menu::Item::Button(label, None, action)
            } else {
                menu::Item::ButtonDisabled(label, None, action)
            }
        };

        let menu_bar = menu::bar(vec![
            menu::Tree::with_children(
                menu::root(fl!("file")).apply(Element::from),
                menu::items(
                    &self.key_binds,
                    vec![
                        menu::Item::Button(fl!("new-file"), None, MenuAction::New),
                        menu::Item::Button(fl!("open-file"), None, MenuAction::Open),
                        menu::Item::Divider,
                        menu::Item::Button(fl!("save"), None, MenuAction::Save),
                        menu::Item::Button(fl!("save-as"), None, MenuAction::SaveAs),
                    ],
                ),
            ),
            menu::Tree::with_children(
                menu::root(fl!("edit")).apply(Element::from),
                menu::items(
                    &self.key_binds,
                    vec![
                        history_item(
                            fl!("undo"),
                            MenuAction::Undo,
                            !self.undo_stack.is_empty(),
                        ),
                        history_item(
                            fl!("redo"),
                            MenuAction::Redo,
                            !self.redo_stack.is_empty(),
                        ),
                        menu::Item::Divider,
                        edit_item(fl!("cut"), MenuAction::Cut),
                        edit_item(fl!("copy"), MenuAction::Copy),
                        edit_item(fl!("paste"), MenuAction::Paste),
                        menu::Item::Divider,
                        edit_item(fl!("select-all"), MenuAction::SelectAll),
                        menu::Item::Divider,
                        edit_item(fl!("find"), MenuAction::Find),
                        edit_item(fl!("replace"), MenuAction::Replace),
                    ],
                ),
            ),
            menu::Tree::with_children(
                menu::root(fl!("view")).apply(Element::from),
                menu::items(
                    &self.key_binds,
                    vec![
                        menu::Item::Button(
                            fl!("toggle-preview"),
                            None,
                            MenuAction::ToggleMode,
                        ),
                        menu::Item::Button(
                            fl!("toggle-headerbar"),
                            None,
                            MenuAction::ToggleHeaderBar,
                        ),
                        menu::Item::Divider,
                        menu::Item::Button(fl!("zoom-in"), None, MenuAction::ZoomIn),
                        menu::Item::Button(fl!("zoom-out"), None, MenuAction::ZoomOut),
                        menu::Item::Button(fl!("zoom-reset"), None, MenuAction::ZoomReset),
                        menu::Item::Divider,
                        menu::Item::Button(fl!("about"), None, MenuAction::About),
                    ],
                ),
            ),
        ])
        .item_height(menu::ItemHeight::Dynamic(40))
        .item_width(menu::ItemWidth::Uniform(240))
        .spacing(4.0);

        vec![menu_bar.into()]
    }

    /// Elements to pack at the end of the header bar.
    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        // A single toggle: in Source mode it offers a preview (eye); in View
        // mode it offers editing (pencil).
        let (icon_bytes, tooltip_label) = match self.document.mode {
            Mode::Source => (ICON_PREVIEW, fl!("show-preview")),
            Mode::View => (ICON_EDIT, fl!("edit-source")),
        };

        let button = widget::button::icon(
            widget::icon::from_svg_bytes(icon_bytes).symbolic(true),
        )
        .class(cosmic::theme::Button::Icon)
        .on_press(Message::ToggleMode);

        let toggle = widget::tooltip(
            button,
            widget::text(tooltip_label),
            widget::tooltip::Position::Bottom,
        );

        vec![toggle.into()]
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::about(
                &self.about,
                |url| Message::LaunchUrl(url.to_string()),
                Message::ToggleContextPage(ContextPage::About),
            ),
        })
    }

    /// Displays a modal dialog over the application when one is active.
    fn dialog(&self) -> Option<Element<'_, Self::Message>> {
        let dialog = match self.dialog.as_ref()? {
            Dialog::ConfirmDiscard(_) => widget::dialog()
                .title(fl!("unsaved-title"))
                .body(fl!("unsaved-body"))
                .primary_action(
                    widget::button::suggested(fl!("save")).on_press(Message::DialogSave),
                )
                .secondary_action(
                    widget::button::destructive(fl!("discard")).on_press(Message::DialogDiscard),
                )
                .tertiary_action(
                    widget::button::text(fl!("cancel")).on_press(Message::DialogCancel),
                ),
            Dialog::ConflictReload(_) => widget::dialog()
                .title(fl!("conflict-title"))
                .body(fl!("conflict-body"))
                .primary_action(
                    widget::button::suggested(fl!("reload-from-disk"))
                        .on_press(Message::ReloadFromDisk),
                )
                .secondary_action(
                    widget::button::standard(fl!("keep-mine")).on_press(Message::KeepLocal),
                ),
        };

        Some(dialog.into())
    }

    /// Closes the find/replace bar when Escape is pressed.
    fn on_escape(&mut self) -> Task<cosmic::Action<Self::Message>> {
        self.search.active = false;
        Task::none()
    }

    /// Called when a window requests to close; vetoes the close to prompt when dirty.
    fn on_close_requested(&self, _id: cosmic::iced::window::Id) -> Option<Self::Message> {
        if self.quitting || !self.document.dirty {
            None
        } else {
            Some(Message::RequestQuit)
        }
    }

    /// Describes the interface based on the current state of the application model.
    ///
    /// Application events will be processed through the view. Any messages emitted by
    /// events received by widgets will be passed to the update method.
    fn view(&self) -> Element<'_, Self::Message> {
        let space_s = cosmic::theme::spacing().space_s;

        let content: Element<_> = match self.document.mode {
            Mode::Source => {
                // Highlight matches only while the find bar is open.
                let query = if self.search.active {
                    self.search.query.clone()
                } else {
                    String::new()
                };

                widget::text_editor(&self.document.content)
                    .id(editor_id())
                    .placeholder(fl!("editor-placeholder"))
                    .on_action(Message::Edit)
                    .height(Length::Fill)
                    .padding(space_s)
                    .size(EDITOR_TEXT_SIZE + self.zoom)
                    .font(Font::MONOSPACE)
                    .class(cosmic::theme::iced::TextEditor::Custom(Box::new(
                        source_editor_style,
                    )))
                    .highlight_with::<SearchHighlighter>(query, search_format)
                    .into()
            }
            Mode::View => widget::container(
                widget::scrollable(
                    markdown::view(
                        self.markdown.items(),
                        markdown_settings(VIEW_TEXT_SIZE + self.zoom),
                    )
                    .map(Message::LaunchUrl),
                )
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .class(cosmic::theme::Container::Custom(Box::new(surface_style)))
            .padding(space_s)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        };

        let mut column = widget::column::with_capacity(4).spacing(space_s);

        if let Some(error) = self.error.as_deref() {
            column = column.push(widget::warning(error).on_close(Message::DismissError));
        }

        if self.search.active && self.document.mode == Mode::Source {
            column = column.push(self.find_bar());
        }

        column = column.push(content);

        // The header bar already separates itself from the content, so use a
        // smaller top margin when it is shown; keep a full margin otherwise.
        let space_xxs = cosmic::theme::spacing().space_xxs;
        let top = if self.core.window.show_headerbar {
            space_xxs
        } else {
            space_s
        };
        let padding = [
            f32::from(top),
            f32::from(space_s - 7),
            f32::from(space_s),
            f32::from(space_s),
        ];

        widget::container(column)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(padding)
            .into()
    }

    /// Register subscriptions for this application.
    ///
    /// Subscriptions are long-running async tasks running in the background which
    /// emit messages to the application through a channel. They can be dynamically
    /// stopped and started conditionally based on application state, or persist
    /// indefinitely.
    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subscriptions = vec![
            // Watch for application configuration changes.
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
            // Listen for keyboard shortcuts.
            keyboard::listen().map(Message::Key),
        ];

        // Watch the open file for external modifications. Keyed by path, so the
        // watcher is re-armed automatically when the open document changes.
        if let Some(path) = self.document.path.clone() {
            subscriptions.push(Subscription::run_with(path, file_watch));
        }

        Subscription::batch(subscriptions)
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the background
    /// on the application's async runtime.
    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::Edit(action) => {
                let is_edit = action.is_edit();

                if is_edit {
                    let run = match &action {
                        text_editor::Action::Edit(text_editor::Edit::Insert(c))
                            if !c.is_whitespace() =>
                        {
                            EditRun::Insert
                        }
                        text_editor::Action::Edit(text_editor::Edit::Backspace) => {
                            EditRun::Backspace
                        }
                        _ => EditRun::None,
                    };
                    // Start a new undo step at run boundaries; coalesce within a run.
                    if run == EditRun::None || run != self.undo_run {
                        self.push_undo_snapshot();
                    }
                    self.undo_run = run;
                } else {
                    self.undo_run = EditRun::None;
                }

                self.document.content.perform(action);

                if is_edit {
                    self.document.dirty = true;
                    self.reparse_markdown();
                }
            }

            Message::ToggleMode => {
                let mode = match self.document.mode {
                    Mode::Source => Mode::View,
                    Mode::View => Mode::Source,
                };
                // Ensure the rendered View reflects the latest source buffer.
                if mode == Mode::View {
                    self.reparse_markdown();
                }
                self.document.mode = mode;
                // Focus the editor when entering Source so the user can type.
                if mode == Mode::Source {
                    return focus_editor();
                }
            }

            Message::ToggleHeaderBar => {
                self.core.window.show_headerbar = !self.core.window.show_headerbar;
            }

            Message::New => {
                return self.guard_or_perform(PendingAction::New);
            }

            Message::OpenFile => {
                return self.guard_or_perform(PendingAction::Open);
            }

            Message::FileOpened(path, contents) => {
                self.document.path = Some(path);
                self.document.mode = Mode::View;
                self.load_contents(&contents);
                self.error = None;
                return self.update_title();
            }

            Message::SaveFile => {
                return self.start_save();
            }

            Message::SaveFileAs => {
                let contents = self.document.content.text();
                let line_ending = self.document.line_ending;
                return cosmic::task::future(save_file_dialog(contents, line_ending));
            }

            Message::FileSaved(path) => {
                self.document.path = Some(path);
                self.document.dirty = false;
                self.error = None;
                // If a save was requested as part of a pending action, continue it.
                if let Some(action) = self.pending.take() {
                    return self.perform_pending(action);
                }
                return self.update_title();
            }

            Message::DialogCancelled => {
                // A pending save was cancelled; abort the deferred action.
                self.pending = None;
            }

            Message::DialogError(why) => {
                self.pending = None;
                self.error = Some(why);
            }

            Message::DismissError => {
                self.error = None;
            }

            Message::RequestQuit => {
                self.dialog = Some(Dialog::ConfirmDiscard(PendingAction::Quit));
            }

            Message::DialogSave => {
                if let Some(Dialog::ConfirmDiscard(action)) = self.dialog.take() {
                    self.pending = Some(action);
                    return self.start_save();
                }
            }

            Message::DialogDiscard => {
                if let Some(Dialog::ConfirmDiscard(action)) = self.dialog.take() {
                    return self.perform_pending(action);
                }
            }

            // Dismiss the active dialog without taking action.
            Message::DialogCancel | Message::KeepLocal => {
                self.dialog = None;
            }

            Message::FileChangedOnDisk => {
                // Don't interrupt an open dialog; re-read the file otherwise.
                if self.dialog.is_none()
                    && let Some(path) = self.document.path.clone()
                {
                    return cosmic::task::future(read_external(path));
                }
            }

            Message::ExternalContent(Ok(disk)) => {
                let current = self.document.content.text();
                if normalized(&disk) == normalized(&current) {
                    // No real change (e.g. our own save) — ignore.
                } else if self.document.dirty {
                    self.dialog = Some(Dialog::ConflictReload(disk));
                } else {
                    self.load_contents(&disk);
                }
            }

            // A transient read failure (e.g. the file was momentarily removed
            // during an atomic save) is ignored; the next event will retry.
            Message::ExternalContent(Err(_)) => {}

            Message::ReloadFromDisk => {
                if let Some(Dialog::ConflictReload(contents)) = self.dialog.take() {
                    self.load_contents(&contents);
                }
            }

            Message::Key(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            }) => {
                if let Some(message) = self.key_binds.iter().find_map(|(bind, action)| {
                    bind.matches(modifiers, &key, Some(&physical_key))
                        .then(|| action.message())
                }) {
                    return self.update(message);
                }
            }

            Message::Key(_) => {}

            Message::Copy => {
                if let Some(selection) = self.document.content.selection() {
                    return cosmic::iced::clipboard::write(selection);
                }
            }

            Message::Cut => {
                if let Some(selection) = self.document.content.selection() {
                    self.begin_edit();
                    self.document
                        .content
                        .perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                    self.document.dirty = true;
                    self.reparse_markdown();
                    return cosmic::iced::clipboard::write(selection);
                }
            }

            Message::Paste => {
                return cosmic::iced::clipboard::read()
                    .map(|contents| cosmic::Action::App(Message::Pasted(contents)));
            }

            Message::Pasted(Some(text)) => {
                self.begin_edit();
                self.document
                    .content
                    .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                        Arc::new(text),
                    )));
                self.document.dirty = true;
                self.reparse_markdown();
            }

            Message::Pasted(None) => {}

            Message::SelectAll => {
                self.document.content.perform(text_editor::Action::SelectAll);
            }

            Message::FindOpen => {
                // Toggle off when already showing the find-only bar; otherwise
                // open/switch to find (collapsing the replace row).
                if self.search.active && !self.search.show_replace {
                    self.search.active = false;
                } else {
                    self.open_search(false);
                    return focus_find();
                }
            }

            Message::ReplaceOpen => {
                // Toggle off when already showing replace; otherwise open/switch
                // to the replace bar.
                if self.search.active && self.search.show_replace {
                    self.search.active = false;
                } else {
                    self.open_search(true);
                    return focus_find();
                }
            }

            Message::FindClose => {
                self.search.active = false;
            }

            Message::FindQueryChanged(query) => {
                self.search.query = query;
                self.search.current = 0;
                self.recompute_matches();
                self.select_current_match();
            }

            Message::ReplacementChanged(replacement) => {
                self.search.replacement = replacement;
            }

            Message::FindNext => {
                if !self.search.matches.is_empty() {
                    self.search.current =
                        (self.search.current + 1) % self.search.matches.len();
                    self.select_current_match();
                }
            }

            Message::FindPrev => {
                if !self.search.matches.is_empty() {
                    let len = self.search.matches.len();
                    self.search.current = (self.search.current + len - 1) % len;
                    self.select_current_match();
                }
            }

            Message::ReplaceCurrent => {
                if !self.search.matches.is_empty() {
                    self.select_current_match();
                    self.begin_edit();
                    let replacement = self.search.replacement.clone();
                    self.document
                        .content
                        .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                            Arc::new(replacement),
                        )));
                    self.document.dirty = true;
                    self.reparse_markdown();
                    self.recompute_matches();
                    self.select_current_match();
                }
            }

            Message::ReplaceAll => {
                if !self.search.matches.is_empty() {
                    self.begin_edit();
                    let replaced = self
                        .document
                        .content
                        .text()
                        .replace(&self.search.query, &self.search.replacement);
                    self.document.content = text_editor::Content::with_text(&replaced);
                    self.document.dirty = true;
                    self.reparse_markdown();
                    self.search.current = 0;
                    self.recompute_matches();
                }
            }

            Message::Undo => {
                if let Some(previous) = self.undo_stack.pop() {
                    self.redo_stack.push(self.document.content.text());
                    self.set_buffer(&previous);
                    self.undo_run = EditRun::None;
                }
            }

            Message::Redo => {
                if let Some(next) = self.redo_stack.pop() {
                    self.undo_stack.push(self.document.content.text());
                    self.set_buffer(&next);
                    self.undo_run = EditRun::None;
                }
            }

            Message::ZoomIn => {
                self.zoom = (self.zoom + ZOOM_STEP).min(ZOOM_MAX);
            }

            Message::ZoomOut => {
                self.zoom = (self.zoom - ZOOM_STEP).max(ZOOM_MIN);
            }

            Message::ZoomReset => {
                self.zoom = 0.0;
            }

            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    // Close the context drawer if the toggled context page is the same.
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    // Open the context drawer to display the requested context page.
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }

            Message::UpdateConfig(config) => {
                self.config = config;
            }

            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("failed to open {url:?}: {err}");
                }
            },
        }
        Task::none()
    }
}

impl AppModel {
    /// Rebuilds the rendered Markdown from the current source buffer.
    fn reparse_markdown(&mut self) {
        self.markdown = markdown::Content::parse(&self.document.content.text());
    }

    /// Opens (or re-targets) the find bar, optionally with the replace row.
    fn open_search(&mut self, show_replace: bool) {
        self.document.mode = Mode::Source;
        self.search.active = true;
        self.search.show_replace = show_replace;
        self.recompute_matches();
        self.select_current_match();
    }

    /// Pushes the current buffer onto the undo stack and clears the redo stack.
    fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(self.document.content.text());
        if self.undo_stack.len() > UNDO_LIMIT {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Records an undo snapshot for a programmatic (non-typing) edit.
    fn begin_edit(&mut self) {
        self.push_undo_snapshot();
        self.undo_run = EditRun::None;
    }

    /// Replaces the buffer with `text` (marking it dirty) without recording undo.
    fn set_buffer(&mut self, text: &str) {
        self.document.content = text_editor::Content::with_text(text);
        self.document.dirty = true;
        self.reparse_markdown();
        self.recompute_matches();
    }

    /// Clears the undo/redo history (e.g. when a new document is loaded).
    fn reset_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.undo_run = EditRun::None;
    }

    /// Builds the find (and optional replace) bar shown above the editor.
    fn find_bar(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing().space_xxs;
        let has_matches = !self.search.matches.is_empty();

        let count = if self.search.query.is_empty() {
            String::new()
        } else if has_matches {
            format!("{}/{}", self.search.current + 1, self.search.matches.len())
        } else {
            fl!("no-matches")
        };

        let find_row = widget::row::with_capacity(6)
            .spacing(spacing)
            .align_y(Alignment::Center)
            .push(
                widget::text_input(fl!("find-placeholder"), &self.search.query)
                    .id(find_input_id())
                    .on_input(Message::FindQueryChanged)
                    .on_submit(|_| Message::FindNext),
            )
            .push(widget::text(count))
            .push(
                widget::button::text(fl!("match-prev"))
                    .on_press_maybe(has_matches.then_some(Message::FindPrev)),
            )
            .push(
                widget::button::text(fl!("match-next"))
                    .on_press_maybe(has_matches.then_some(Message::FindNext)),
            )
            .push(widget::space::horizontal())
            .push(widget::button::text(fl!("close")).on_press(Message::FindClose));

        let mut column = widget::column::with_capacity(2)
            .spacing(spacing)
            .push(find_row);

        if self.search.show_replace {
            let can_replace = has_matches && !self.search.query.is_empty();
            let replace_row = widget::row::with_capacity(3)
                .spacing(spacing)
                .align_y(Alignment::Center)
                .push(
                    widget::text_input(fl!("replace-placeholder"), &self.search.replacement)
                        .on_input(Message::ReplacementChanged),
                )
                .push(
                    widget::button::text(fl!("replace-one"))
                        .on_press_maybe(can_replace.then_some(Message::ReplaceCurrent)),
                )
                .push(
                    widget::button::text(fl!("replace-all"))
                        .on_press_maybe(can_replace.then_some(Message::ReplaceAll)),
                );
            column = column.push(replace_row);
        }

        widget::container(column)
            .class(cosmic::theme::Container::Custom(Box::new(surface_style)))
            .padding(spacing)
            .into()
    }

    /// Recomputes search matches for the current query over the buffer.
    fn recompute_matches(&mut self) {
        self.search.matches.clear();

        let query = self.search.query.clone();
        if query.is_empty() {
            return;
        }

        let text = self.document.content.text();
        for (line, content) in text.lines().enumerate() {
            let mut from = 0;
            while let Some(offset) = content[from..].find(&query) {
                let start = from + offset;
                let end = start + query.len();
                self.search.matches.push(Match { line, start, end });
                from = end.max(start + 1);
            }
        }

        if self.search.current >= self.search.matches.len() {
            self.search.current = 0;
        }
    }

    /// Selects the active match in the editor, if any.
    fn select_current_match(&mut self) {
        if let Some(m) = self.search.matches.get(self.search.current) {
            self.document.content.move_to(text_editor::Cursor {
                position: text_editor::Position {
                    line: m.line,
                    column: m.end,
                },
                selection: Some(text_editor::Position {
                    line: m.line,
                    column: m.start,
                }),
            });
        }
    }

    /// Replaces the document's buffer with `contents`, marking it clean and
    /// adopting the contents' line-ending convention.
    fn load_contents(&mut self, contents: &str) {
        self.document.line_ending = LineEnding::detect(contents);
        self.document.content = text_editor::Content::with_text(contents);
        self.document.dirty = false;
        self.reparse_markdown();
        self.reset_history();
    }

    /// Performs `action` immediately, or prompts to save first when the document
    /// has unsaved changes.
    fn guard_or_perform(&mut self, action: PendingAction) -> Task<cosmic::Action<Message>> {
        if self.document.dirty {
            self.dialog = Some(Dialog::ConfirmDiscard(action));
            Task::none()
        } else {
            self.perform_pending(action)
        }
    }

    /// Carries out a previously deferred action.
    fn perform_pending(&mut self, action: PendingAction) -> Task<cosmic::Action<Message>> {
        match action {
            PendingAction::New => {
                self.document = Document::default();
                self.markdown = markdown::Content::new();
                self.error = None;
                self.reset_history();
                self.update_title()
            }
            PendingAction::Open => cosmic::task::future(open_file_dialog()),
            PendingAction::Quit => {
                self.quitting = true;
                self.core
                    .main_window_id()
                    .map_or_else(Task::none, cosmic::iced::window::close)
            }
        }
    }

    /// Saves the current document, prompting for a path if it has none.
    fn start_save(&mut self) -> Task<cosmic::Action<Message>> {
        let contents = self.document.content.text();
        let line_ending = self.document.line_ending;
        if let Some(path) = self.document.path.clone() {
            cosmic::task::future(write_file(path, contents, line_ending))
        } else {
            cosmic::task::future(save_file_dialog(contents, line_ending))
        }
    }

    /// Updates the header and window titles.
    pub fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = fl!("app-title");

        if let Some(name) = self
            .document
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
        {
            window_title.push_str(" — ");
            window_title.push_str(&name);
        }

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }
}

/// Styles the Source editor as a distinct input surface so it stands out from
/// the window background (the libcosmic default uses the plain window color).
fn source_editor_style(
    theme: &cosmic::Theme,
    status: text_editor::Status,
) -> text_editor::Style {
    use cosmic::iced::{Border, Color};

    let cosmic = theme.cosmic();
    let value = Color::from(cosmic.on_primary_container_color());
    let mut placeholder = value;
    placeholder.a = 0.7;
    let accent = Color::from(cosmic.accent_color());
    let focused = matches!(status, text_editor::Status::Focused { .. });

    text_editor::Style {
        background: Color::from(cosmic.primary_container_color()).into(),
        border: Border {
            radius: cosmic.corner_radii.radius_s.into(),
            width: 1.0,
            color: if focused {
                accent
            } else {
                Color::from(cosmic.primary_container_divider())
            },
        },
        placeholder,
        value,
        selection: accent,
    }
}

/// Styles a container as the same input surface as the Source editor, so the
/// rendered View matches the editor's appearance.
fn surface_style(theme: &cosmic::Theme) -> cosmic::widget::container::Style {
    use cosmic::iced::{Border, Color};

    let cosmic = theme.cosmic();

    cosmic::widget::container::Style {
        background: Some(Color::from(cosmic.primary_container_color()).into()),
        border: Border {
            radius: cosmic.corner_radii.radius_s.into(),
            width: 1.0,
            color: Color::from(cosmic.primary_container_divider()),
        },
        ..Default::default()
    }
}

/// Builds Markdown render settings from the active COSMIC theme (light/dark).
fn markdown_settings(text_size: f32) -> markdown::Settings {
    let theme = if cosmic::theme::is_dark() {
        cosmic::iced::Theme::Dark
    } else {
        cosmic::iced::Theme::Light
    };
    markdown::Settings::with_text_size(text_size, markdown::Style::from(&theme))
}

/// The context page to display in the context drawer.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    Replace,
    ToggleMode,
    ToggleHeaderBar,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::New => Message::New,
            MenuAction::Open => Message::OpenFile,
            MenuAction::Save => Message::SaveFile,
            MenuAction::SaveAs => Message::SaveFileAs,
            MenuAction::Undo => Message::Undo,
            MenuAction::Redo => Message::Redo,
            MenuAction::Cut => Message::Cut,
            MenuAction::Copy => Message::Copy,
            MenuAction::Paste => Message::Paste,
            MenuAction::SelectAll => Message::SelectAll,
            MenuAction::Find => Message::FindOpen,
            MenuAction::Replace => Message::ReplaceOpen,
            MenuAction::ToggleMode => Message::ToggleMode,
            MenuAction::ToggleHeaderBar => Message::ToggleHeaderBar,
            MenuAction::ZoomIn => Message::ZoomIn,
            MenuAction::ZoomOut => Message::ZoomOut,
            MenuAction::ZoomReset => Message::ZoomReset,
        }
    }
}

/// The widget id of the find query input (for focusing).
fn find_input_id() -> widget::Id {
    widget::Id::new("cosmic-markdown-find-input")
}

/// The widget id of the Source editor (for focusing).
fn editor_id() -> widget::Id {
    widget::Id::new("cosmic-markdown-editor")
}

/// A task that focuses the find query input.
fn focus_find() -> Task<cosmic::Action<Message>> {
    widget::text_input::focus(find_input_id()).map(cosmic::Action::App)
}

/// A task that focuses the Source editor.
fn focus_editor() -> Task<cosmic::Action<Message>> {
    cosmic::iced::advanced::widget::operate(
        cosmic::iced::advanced::widget::operation::focusable::focus(editor_id()),
    )
}

/// Highlights occurrences of a query string in the editor (recolours matches).
struct SearchHighlighter {
    query: String,
    line: usize,
}

impl cosmic::iced::advanced::text::Highlighter for SearchHighlighter {
    type Settings = String;
    type Highlight = ();
    type Iterator<'a> = std::vec::IntoIter<(std::ops::Range<usize>, ())>;

    fn new(settings: &Self::Settings) -> Self {
        Self {
            query: settings.clone(),
            line: 0,
        }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.query.clone_from(new_settings);
        self.line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let mut ranges = Vec::new();
        if !self.query.is_empty() {
            let mut from = 0;
            while let Some(offset) = line[from..].find(&self.query) {
                let start = from + offset;
                let end = start + self.query.len();
                ranges.push((start..end, ()));
                from = end.max(start + 1);
            }
        }
        self.line += 1;
        ranges.into_iter()
    }

    fn current_line(&self) -> usize {
        self.line
    }
}

/// Maps a search highlight to a distinct text colour (the accent colour).
// Signature is dictated by `text_editor::highlight_with`'s `fn(&H::Highlight, &Theme)`.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn search_format(
    _highlight: &(),
    theme: &cosmic::Theme,
) -> cosmic::iced::advanced::text::highlighter::Format<cosmic::iced::Font> {
    cosmic::iced::advanced::text::highlighter::Format {
        color: Some(cosmic::iced::Color::from(theme.cosmic().accent_color())),
        font: None,
    }
}

/// The application's keyboard shortcuts, mapped to menu actions.
fn key_binds() -> HashMap<menu::KeyBind, MenuAction> {
    use keyboard::Key;
    use menu::key_bind::Modifier;

    let mut binds = HashMap::new();

    macro_rules! bind {
        ([$($modifier:ident),*], $key:expr, $action:ident) => {
            binds.insert(
                menu::KeyBind {
                    modifiers: vec![$(Modifier::$modifier),*],
                    key: $key,
                },
                MenuAction::$action,
            );
        };
    }

    bind!([Ctrl], Key::Character("n".into()), New);
    bind!([Ctrl], Key::Character("o".into()), Open);
    bind!([Ctrl], Key::Character("s".into()), Save);
    bind!([Ctrl, Shift], Key::Character("s".into()), SaveAs);
    bind!([Ctrl], Key::Character("e".into()), ToggleMode);
    bind!([Ctrl], Key::Character("a".into()), SelectAll);
    bind!([Ctrl], Key::Character("f".into()), Find);
    bind!([Ctrl], Key::Character("h".into()), Replace);
    bind!([Ctrl], Key::Character("z".into()), Undo);
    bind!([Ctrl, Shift], Key::Character("z".into()), Redo);
    bind!([Ctrl], Key::Character("y".into()), Redo);
    bind!([Ctrl, Shift], Key::Character("h".into()), ToggleHeaderBar);
    // Zoom: cover Ctrl+=, Ctrl++ (Shift+=), numpad +, Ctrl+-, and Ctrl+0.
    bind!([Ctrl], Key::Character("=".into()), ZoomIn);
    bind!([Ctrl, Shift], Key::Character("=".into()), ZoomIn);
    bind!([Ctrl], Key::Character("+".into()), ZoomIn);
    bind!([Ctrl], Key::Character("-".into()), ZoomOut);
    bind!([Ctrl], Key::Character("0".into()), ZoomReset);

    binds
}

/// A file filter matching Markdown documents.
fn markdown_filter() -> FileFilter {
    FileFilter::new(&fl!("markdown-files"))
        .glob("*.md")
        .glob("*.markdown")
        .glob("*.mdown")
        .glob("*.mkd")
}

/// Prompts for a Markdown file and reads it into memory.
async fn open_file_dialog() -> Message {
    let dialog = file_chooser::open::Dialog::new()
        .title(fl!("open-file"))
        .filter(markdown_filter());

    match dialog.open_file().await {
        Ok(response) => match response.url().to_file_path() {
            Ok(path) => match tokio::fs::read_to_string(&path).await {
                Ok(contents) => Message::FileOpened(path, contents),
                Err(why) => {
                    Message::DialogError(format!("failed to read {}: {why}", path.display()))
                }
            },
            Err(()) => Message::DialogError("selected file is not a local path".into()),
        },
        Err(file_chooser::Error::Cancelled) => Message::DialogCancelled,
        Err(why) => Message::DialogError(why.to_string()),
    }
}

/// Prompts for a destination path, then writes the contents there.
async fn save_file_dialog(contents: String, line_ending: LineEnding) -> Message {
    let dialog = file_chooser::save::Dialog::new()
        .title(fl!("save-as"))
        .filter(markdown_filter());

    match dialog.save_file().await {
        Ok(response) => match response.url() {
            Some(url) => match url.to_file_path() {
                Ok(path) => write_file(path, contents, line_ending).await,
                Err(()) => Message::DialogError("selected file is not a local path".into()),
            },
            None => Message::DialogCancelled,
        },
        Err(file_chooser::Error::Cancelled) => Message::DialogCancelled,
        Err(why) => Message::DialogError(why.to_string()),
    }
}

/// Writes `contents` to `path` using the document's line-ending convention.
async fn write_file(path: PathBuf, contents: String, line_ending: LineEnding) -> Message {
    match tokio::fs::write(&path, line_ending.apply(contents)).await {
        Ok(()) => Message::FileSaved(path),
        Err(why) => Message::DialogError(format!("failed to save {}: {why}", path.display())),
    }
}

/// Re-reads a file after it changed on disk.
async fn read_external(path: PathBuf) -> Message {
    Message::ExternalContent(
        tokio::fs::read_to_string(&path)
            .await
            .map_err(|why| why.to_string()),
    )
}

/// Normalizes line endings to `\n` for content comparison.
fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// A subscription stream that emits a message whenever `path` changes on disk.
//
// `&PathBuf` (not `&Path`) is required to match `Subscription::run_with`'s
// `fn(&D)` builder signature, where the keying data `D` is a `PathBuf`.
#[allow(clippy::ptr_arg)]
fn file_watch(path: &PathBuf) -> Pin<Box<dyn Stream<Item = Message> + Send>> {
    let path = path.clone();

    Box::pin(cosmic::iced::stream::channel(
        16,
        move |mut output: cosmic::iced::futures::channel::mpsc::Sender<Message>| async move {
        use notify::Watcher;

        // notify invokes its handler on a background thread; forward relevant
        // events through a channel into this async task.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let Ok(mut watcher) = notify::recommended_watcher(
            move |result: notify::Result<notify::Event>| {
                if let Ok(event) = result
                    && matches!(
                        event.kind,
                        notify::EventKind::Modify(_)
                            | notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                    )
                {
                    let _ = tx.send(());
                }
            },
        ) else {
            return;
        };

        if watcher
            .watch(&path, notify::RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }

        while rx.recv().await.is_some() {
            if output.send(Message::FileChangedOnDisk).await.is_err() {
                break;
            }
        }

        // Keep the watcher alive until the stream ends.
        drop(watcher);
        },
    ))
}
