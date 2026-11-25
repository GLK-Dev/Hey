// Copyright (c) Vitaly Golik (mjojo)

//! Native dialogs (`rfd`). They block the calling thread — run via `Command::perform`.

use std::path::PathBuf;

/// Pick a single file; `filters` are (name, extensions without the dot) pairs.
pub fn open_file(title: &str, filters: &[(&str, &[&str])]) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(title);
    for (name, exts) in filters {
        d = d.add_filter(*name, exts);
    }
    d.pick_file()
}

pub fn open_files(title: &str) -> Vec<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_files().unwrap_or_default()
}

pub fn save_file(title: &str, name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).set_file_name(name).save_file()
}

pub fn pick_folder(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_folder()
}

/// Message with an OK button.
pub fn message(title: &str, text: &str) {
    rfd::MessageDialog::new().set_title(title).set_description(text).show();
}

/// Yes/No question.
pub fn confirm(title: &str, text: &str) -> bool {
    rfd::MessageDialog::new().set_title(title).set_description(text).set_buttons(rfd::MessageButtons::YesNo).show()
        == rfd::MessageDialogResult::Yes
}
