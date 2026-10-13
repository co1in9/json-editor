#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod json;

use gpui_kit::{
    component::{
        button::*,
        input::{Editor, EditorState, InputEvent, TabSize},
        *,
    },
    *,
};

const SAMPLE: &str = r#"{
  "name": "JSON Editor",
  "version": 1,
  "description": "自由编辑你的 JSON",
  "settings": {
    "enabled": true,
    "theme": "light",
    "indent": 2
  },
  "items": [
    { "id": 1, "price": 29.9 },
    { "id": 2, "price": 49.5 }
  ]
}"#;

actions!(json_editor, [FormatJson, ValidateJson, CopyDocument, PasteDocument]);

#[cfg(target_os = "macos")]
const SHORTCUT_MODIFIER: &str = "cmd";
#[cfg(not(target_os = "macos"))]
const SHORTCUT_MODIFIER: &str = "ctrl";

#[cfg(target_os = "macos")]
const SHORTCUT_LABEL: &str = "Cmd";
#[cfg(not(target_os = "macos"))]
const SHORTCUT_LABEL: &str = "Ctrl";

fn bind_shortcuts(cx: &mut App) {
    // Input has its own bindings (e.g. Cmd+Shift+F opens search/replace).
    // Match at the editor's depth as well so document actions take precedence.
    for context in ["JsonEditor", "JsonEditor > Input"] {
        cx.bind_keys([
            KeyBinding::new(
                &format!("{SHORTCUT_MODIFIER}-shift-f"),
                FormatJson,
                Some(context),
            ),
            KeyBinding::new(
                &format!("{SHORTCUT_MODIFIER}-enter"),
                ValidateJson,
                Some(context),
            ),
            KeyBinding::new(
                &format!("{SHORTCUT_MODIFIER}-shift-c"),
                CopyDocument,
                Some(context),
            ),
            KeyBinding::new(
                &format!("{SHORTCUT_MODIFIER}-shift-v"),
                PasteDocument,
                Some(context),
            ),
        ]);
    }
}

struct JsonEditor {
    editor: Entity<EditorState>,
    status: String,
    error: bool,
    _subscriptions: Vec<Subscription>,
    operation_source: Option<String>,
}

impl JsonEditor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
                .folding(true)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .default_value(SAMPLE)
        });
        let subscription = cx.subscribe(&editor, |this, state, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                if this
                    .operation_source
                    .take()
                    .is_some_and(|source| source == state.read(cx).value().as_str())
                {
                    cx.notify();
                    return;
                }
                this.status = "已修改 · 点击验证检查 JSON".into();
                this.error = false;
                cx.notify();
            }
        });
        editor.update(cx, |state, cx| state.focus(window, cx));
        Self {
            editor,
            status: "就绪 · 点击行号旁的箭头折叠对象或数组".into(),
            error: false,
            _subscriptions: vec![subscription],
            operation_source: None,
        }
    }

    fn format(&mut self, _: &FormatJson, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.editor.read(cx).value().to_string();
        match json::format(&source) {
            Ok(formatted) => {
                self.operation_source = Some(formatted.clone());
                self.editor
                    .update(cx, |state, cx| state.replace_all(formatted, window, cx));
                self.status = "已格式化 · 2 空格缩进".into();
                self.error = false;
            }
            Err(error) => self.report(error),
        }
        cx.notify();
    }

    fn validate(&mut self, _: &ValidateJson, _: &mut Window, cx: &mut Context<Self>) {
        match json::validate(&self.editor.read(cx).value().to_string()) {
            Ok(()) => {
                self.status = "验证通过 · JSON 格式正确".into();
                self.error = false;
            }
            Err(error) => self.report(error),
        }
        cx.notify();
    }

    fn copy_document(&mut self, _: &CopyDocument, _: &mut Window, cx: &mut Context<Self>) {
        let source = self.editor.read(cx).value().to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(source));
        self.status = "已复制整份文档到剪贴板".into();
        self.error = false;
        cx.notify();
    }

    fn paste_document(&mut self, _: &PasteDocument, window: &mut Window, cx: &mut Context<Self>) {
        match cx.read_from_clipboard().and_then(|item| item.text()) {
            Some(source) if !source.is_empty() => {
                self.operation_source = Some(source.clone());
                self.editor.update(cx, |state, cx| {
                    state.replace_all(source, window, cx);
                    state.focus(window, cx);
                });
                self.status = "已从剪贴板替换整份文档 · 可撤销 · 待验证".into();
                self.error = false;
            }
            _ => {
                self.status = "剪贴板为空、无法读取或不包含文本 · 文档未修改".into();
                self.error = true;
            }
        }
        cx.notify();
    }

    fn report(&mut self, error: json::JsonError) {
        self.status = format!(
            "第 {} 行，第 {} 列：{}",
            error.line, error.column, error.message
        );
        self.error = true;
    }
}

impl Render for JsonEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = self.editor.read(cx).value();
        let lines = source.lines().count().max(1);
        let bytes = source.len();
        div()
            .key_context("JsonEditor")
            .size_full()
            .v_flex()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::format))
            .on_action(cx.listener(Self::validate))
            .on_action(cx.listener(Self::copy_document))
            .on_action(cx.listener(Self::paste_document))
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .gap_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().text_lg().font_semibold().child("JSON Editor"))
                    .child(div().flex_1())
                    .child(
                        Button::new("copy-document")
                            .label("复制")
                            .tooltip_with_action("复制整份文档", &CopyDocument, Some("JsonEditor"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.copy_document(&CopyDocument, window, cx)
                            })),
                    )
                    .child(
                        Button::new("paste-document")
                            .label("从剪贴板粘贴")
                            .tooltip_with_action(
                                "从剪贴板替换整份文档（可撤销）",
                                &PasteDocument,
                                Some("JsonEditor"),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.paste_document(&PasteDocument, window, cx)
                            })),
                    )
                    .child(
                        Button::new("format")
                            .label("格式化")
                            .tooltip_with_action("格式化 JSON", &FormatJson, Some("JsonEditor"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.format(&FormatJson, window, cx)
                            })),
                    )
                    .child(
                        Button::new("validate")
                            .primary()
                            .label("验证")
                            .tooltip_with_action("验证 JSON", &ValidateJson, Some("JsonEditor"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.validate(&ValidateJson, window, cx)
                            })),
                    ),
            )
            .child(
                h_flex()
                    .px_6()
                    .py_2()
                    .justify_between()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("document.json")
                    .child(format!(
                        "{SHORTCUT_LABEL}+Shift+F 格式化 · {SHORTCUT_LABEL}+Enter 验证"
                    )),
            )
            .child(
                div().flex_1().min_h_0().px_4().pb_3().child(
                    Editor::new(&self.editor)
                        .h_full()
                        .w_full()
                        .bordered(false)
                        .text_size(px(15.)),
                ),
            )
            .child(
                h_flex()
                    .px_6()
                    .py_3()
                    .gap_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .child(
                        div()
                            .flex_1()
                            .text_color(if self.error {
                                cx.theme().danger
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(self.status.clone()),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{lines} 行 · {bytes} 字节 · JSON")),
                    ),
            )
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            bind_shortcuts(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1000.), px(720.)),
                        cx,
                    ))),
                    window_min_size: Some(size(px(640.), px(400.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("JSON Editor".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| JsonEditor::new(window, cx)),
            )
            .expect("无法创建 JSON 编辑器窗口");
            cx.activate(true);
        });
}

#[cfg(test)]
mod ui_tests {
    use super::{FormatJson, JsonEditor, SHORTCUT_MODIFIER, ValidateJson, bind_shortcuts};
    use gpui_kit::{
        AppContext, ClipboardItem, TestAppContext, WindowOptions, component::input::Undo,
        test::TestWindowExt,
    };

    #[gpui_kit::test]
    fn clipboard_buttons_shortcuts_and_undo(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            bind_shortcuts(cx);
        });
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| JsonEditor::new(window, cx))
            })
            .unwrap()
        });
        let original = r#"{"hello":"你好","n":42}"#;
        let replacement = "{\n  \"clipboard\": [true, null]\n}";
        cx.update_window(window, |_, window, cx| {
            view.read(cx).editor.clone().update(cx, |state, cx| {
                state.set_value(original, window, cx);
                state.focus(window, cx);
            });
            window.render_frame(cx);
            window.click("copy-document", cx);
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), original);
            cx.write_to_clipboard(ClipboardItem::new_string(replacement.into()));
            window.click("paste-document", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), replacement);
            assert!(view.read(cx).status.starts_with("已从剪贴板"));
            window.dispatch_action(Box::new(Undo), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), original);
            cx.write_to_clipboard(ClipboardItem { entries: vec![] });
            window.click("paste-document", cx);
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), original);
            assert!(view.read(cx).error);
            cx.write_to_clipboard(ClipboardItem::new_string("{invalid JSON".into()));
            view.read(cx)
                .editor
                .clone()
                .update(cx, |state, cx| state.focus(window, cx));
            window.render_frame(cx);
        })
        .unwrap();
        cx.simulate_keystrokes(window.into(), &format!("{SHORTCUT_MODIFIER}-shift-v"));
        cx.simulate_keystrokes(window.into(), &format!("{SHORTCUT_MODIFIER}-shift-c"));
        cx.update_window(window, |_, _, cx| {
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), "{invalid JSON");
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "{invalid JSON");
        })
        .unwrap();
        cx.simulate_keystrokes(window.into(), &format!("{SHORTCUT_MODIFIER}-enter"));
        cx.update_window(window, |_, window, cx| {
            assert!(view.read(cx).error);
            assert!(view.read(cx).status.starts_with("第 1 行"));
            view.read(cx).editor.clone().update(cx, |state, cx| {
                state.set_value(original, window, cx);
                state.focus(window, cx);
            });
            window.render_frame(cx);
        })
        .unwrap();
        cx.simulate_keystrokes(window.into(), &format!("{SHORTCUT_MODIFIER}-shift-f"));
        cx.update_window(window, |_, _, cx| {
            assert!(view.read(cx).status.starts_with("已格式化"));
            assert!(view.read(cx).editor.read(cx).value().contains('\n'));
        })
        .unwrap();
        cx.simulate_keystrokes(window.into(), &format!("{SHORTCUT_MODIFIER}-enter"));
        cx.update_window(window, |_, _, cx| {
            assert!(!view.read(cx).error);
            assert!(view.read(cx).status.starts_with("验证通过"));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn toolbar_formats_validates_and_preserves_undo(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| JsonEditor::new(window, cx))
            })
            .unwrap()
        });
        let source = r#"{"hello":"你好","n":42}"#;
        cx.update_window(window, |_, window, cx| {
            view.read(cx)
                .editor
                .clone()
                .update(cx, |state, cx| state.set_value(source, window, cx));
            window.render_frame(cx);
            window.click("format", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            assert!(view.read(cx).status.starts_with("已格式化"));
            assert!(view.read(cx).editor.read(cx).value().contains('\n'));
            view.read(cx)
                .editor
                .clone()
                .update(cx, |state, cx| state.focus(window, cx));
            window.dispatch_action(Box::new(Undo), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), source);
            window.click("validate", cx);
            assert!(!view.read(cx).error);
            assert!(view.read(cx).status.starts_with("验证通过"));
            let editor = view.read(cx).editor.clone();
            editor.update(cx, |state, cx| {
                state.set_value("{\n  \"x\": ,\n}", window, cx)
            });
            view.update(cx, |state, cx| state.validate(&ValidateJson, window, cx));
            assert!(view.read(cx).error);
            assert!(view.read(cx).status.starts_with("第 2 行，第 8 列"));
            view.update(cx, |state, cx| state.format(&FormatJson, window, cx));
            assert_eq!(editor.read(cx).value().as_str(), "{\n  \"x\": ,\n}");
        })
        .unwrap();
    }
}
