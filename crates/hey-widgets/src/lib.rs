// Copyright (c) Vitaly Golik (mjojo)

//! hey-widgets: виджеты поверх дерева раскладки. Без привязки к окну/GPU:
//! вход — `UiEvent`, выход — `DisplayList` и сообщения приложения (MVU).

mod textarea;
mod ui;
mod widgets;

pub use textarea::TextArea;
pub use ui::{Env, Ui, Widget};
pub use widgets::{Button, Label, Panel, PanelKind, Tabs, TextInput};
