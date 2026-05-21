//! MenuItem — DropdownMenu / ContextMenu 内单项数据。
//!
//! 纯数据 struct，不实现 IntoElement。由 DropdownMenu 等 caller 在 render 时
//! 读取字段绘制行。

use gpui::SharedString;

use crate::icons::IconName;

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: SharedString,
    pub icon: Option<IconName>,
    pub shortcut: Option<SharedString>,
    pub disabled: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, i: IconName) -> Self {
        self.icon = Some(i);
        self
    }

    pub fn shortcut(mut self, s: impl Into<SharedString>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults() {
        let m = MenuItem::new("Open");
        assert_eq!(m.label.as_ref(), "Open");
        assert!(m.icon.is_none());
        assert!(m.shortcut.is_none());
        assert!(!m.disabled);
    }

    #[test]
    fn icon_and_shortcut_chain() {
        let m = MenuItem::new("Save")
            .icon(IconName::Check)
            .shortcut("Ctrl+S");
        assert!(m.icon.is_some());
        assert_eq!(m.shortcut.as_ref().unwrap().as_ref(), "Ctrl+S");
    }

    #[test]
    fn disabled_chain() {
        let m = MenuItem::new("Delete").disabled(true);
        assert!(m.disabled);
    }
}
