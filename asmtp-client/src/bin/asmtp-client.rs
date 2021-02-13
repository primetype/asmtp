use asmtp_client::App;
use iced::{Application as _, Settings};

fn main() {
    let mut settings = Settings::default();

    settings.window.size = (800, 600);
    settings.window.resizable = true;
    settings.window.decorations = true;

    settings.default_font = None;

    App::run(settings).unwrap();
}
