pub mod chat;
pub mod pages;
pub mod renderer;
pub mod text;
pub mod window;
pub mod layout;
pub mod settings;
pub mod settings_window;
mod theme;
pub use theme::set_accent;

pub mod notification_text;
pub mod icon_images;
pub mod glass;
#[cfg(windows)]
mod glass_win;

pub use renderer::phase8_view;
