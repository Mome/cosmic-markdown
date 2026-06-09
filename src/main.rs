// SPDX-License-Identifier: MPL-2.0

mod app;
mod config;
mod i18n;

fn main() -> cosmic::iced::Result {
    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Settings for configuring the application window and iced runtime.
    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(360.0)
            .min_height(180.0),
    );

    // A file path may be passed on the command line (e.g. from "Open with").
    let file = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    // Starts the application's event loop, opening `file` if provided.
    cosmic::app::run::<app::AppModel>(settings, file)
}
