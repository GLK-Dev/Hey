// Copyright (c) Vitaly Golik (mjojo)

//! Нативные диалоги (`rfd`). Блокируют вызывающий поток — запускать через `Command::perform`.

use std::path::PathBuf;

/// Выбрать один файл; `filters` — пары (название, расширения без точки).
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

/// Сообщение с кнопкой OK.
pub fn message(title: &str, text: &str) {
    rfd::MessageDialog::new().set_title(title).set_description(text).show();
}

/// Вопрос Да/Нет.
pub fn confirm(title: &str, text: &str) -> bool {
    rfd::MessageDialog::new().set_title(title).set_description(text).set_buttons(rfd::MessageButtons::YesNo).show()
        == rfd::MessageDialogResult::Yes
}
