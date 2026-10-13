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

actions!(json_editor, [FormatJson, ValidateJson]);

struct JsonEditor {
    editor: Entity<EditorState>,
    status: String,
    error: bool,
    _subscriptions: Vec<Subscription>,
    formatted_source: Option<String>,
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
                    .formatted_source
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
            formatted_source: None,
        }
    }

    fn format(&mut self, _: &FormatJson, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.editor.read(cx).value().to_string();
        match json::format(&source) {
            Ok(formatted) => {
                self.formatted_source = Some(formatted.clone());
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
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .gap_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().text_lg().font_semibold().child("JSON Editor"))
                    .child(div().flex_1())
                    .child(Button::new("format").label("格式化").on_click(
                        cx.listener(|this, _, window, cx| this.format(&FormatJson, window, cx)),
                    ))
                    .child(Button::new("validate").primary().label("验证").on_click(
                        cx.listener(|this, _, window, cx| this.validate(&ValidateJson, window, cx)),
                    )),
            )
            .child(
                h_flex()
                    .px_6()
                    .py_2()
                    .justify_between()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("document.json")
                    .child("Ctrl+Shift+F 格式化   ·   Ctrl+Enter 验证"),
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
            cx.bind_keys([
                KeyBinding::new("ctrl-shift-f", FormatJson, Some("JsonEditor")),
                KeyBinding::new("ctrl-enter", ValidateJson, Some("JsonEditor")),
            ]);
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
    use super::{FormatJson, JsonEditor, ValidateJson};
    use gpui_kit::{
        AppContext, TestAppContext, WindowOptions, component::input::Undo, test::TestWindowExt,
    };

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
