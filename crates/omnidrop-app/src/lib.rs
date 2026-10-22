//! Application model for the OmniDrop desktop shell: user settings, recent
//! history, the payloads exchanged with the UI and queue previews. Kept free
//! of the webview runtime so it can be tested on every platform.

pub mod history;
pub mod payloads;
pub mod settings;
pub mod thumbnails;
