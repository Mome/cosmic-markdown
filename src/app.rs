// SPDX-License-Identifier: MPL-2.0

use crate::config::Config;
use crate::fl;
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::dialog::file_chooser::{self, FileFilter};
use cosmic::iced::futures::{SinkExt, Stream};
use cosmic::iced::{Font, Length, Subscription, keyboard};
use cosmic::prelude::*;
use cosmic::widget::menu::action::MenuAction as _;
use cosmic::widget::{self, about::About, markdown, menu, text_editor};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");

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
    /// Switch the active view mode.
    SetMode(Mode),
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
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = ();

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
        _flags: Self::Flags,
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
            dialog: None,
            pending: None,
            quitting: false,
        };

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (app, command)
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
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
                menu::root(fl!("view")).apply(Element::from),
                menu::items(
                    &self.key_binds,
                    vec![menu::Item::Button(fl!("about"), None, MenuAction::About)],
                ),
            ),
        ]);

        vec![menu_bar.into()]
    }

    /// Elements to pack at the end of the header bar.
    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        let mode = self.document.mode;
        let spacing = cosmic::theme::spacing().space_xxs;

        let class = |active| {
            if active {
                cosmic::theme::Button::Suggested
            } else {
                cosmic::theme::Button::Text
            }
        };

        let toggle = widget::row::with_capacity(2)
            .push(
                widget::button::text(fl!("mode-source"))
                    .class(class(mode == Mode::Source))
                    .on_press(Message::SetMode(Mode::Source)),
            )
            .push(
                widget::button::text(fl!("mode-view"))
                    .class(class(mode == Mode::View))
                    .on_press(Message::SetMode(Mode::View)),
            )
            .spacing(spacing);

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
            Mode::Source => widget::text_editor(&self.document.content)
                .placeholder(fl!("editor-placeholder"))
                .on_action(Message::Edit)
                .height(Length::Fill)
                .padding(space_s)
                .font(Font::MONOSPACE)
                .into(),
            Mode::View => widget::scrollable(
                markdown::view(self.markdown.items(), markdown_settings())
                    .map(Message::LaunchUrl),
            )
            .spacing(space_s)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        };

        let mut column = widget::column::with_capacity(2).spacing(space_s);

        if let Some(error) = self.error.as_deref() {
            column = column.push(widget::warning(error).on_close(Message::DismissError));
        }

        column = column.push(content);

        widget::container(column)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(space_s)
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
                self.document.content.perform(action);

                if is_edit {
                    self.document.dirty = true;
                    self.reparse_markdown();
                }
            }

            Message::SetMode(mode) => {
                // Ensure the rendered View reflects the latest source buffer.
                if mode == Mode::View {
                    self.reparse_markdown();
                }
                self.document.mode = mode;
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

    /// Replaces the document's buffer with `contents`, marking it clean and
    /// adopting the contents' line-ending convention.
    fn load_contents(&mut self, contents: &str) {
        self.document.line_ending = LineEnding::detect(contents);
        self.document.content = text_editor::Content::with_text(contents);
        self.document.dirty = false;
        self.reparse_markdown();
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

/// Builds Markdown render settings from the active COSMIC theme (light/dark).
fn markdown_settings() -> markdown::Settings {
    let theme = if cosmic::theme::is_dark() {
        cosmic::iced::Theme::Dark
    } else {
        cosmic::iced::Theme::Light
    };
    markdown::Settings::from(&theme)
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
        }
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
