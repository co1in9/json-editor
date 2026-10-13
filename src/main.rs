#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod documents;
mod json;

use documents::DocumentStore;
use std::io;

use gpui_kit::{
    component::{
        button::*,
        input::{Editor, EditorState, Input, InputEvent, InputState, TabSize},
        *,
    },
    prelude::FluentBuilder,
    *,
};

actions!(
    json_editor,
    [
        FormatJson,
        ValidateJson,
        CopyDocument,
        PasteDocument,
        SaveDocument
    ]
);

#[derive(Clone)]
enum DocumentDestination {
    New,
    History(String),
}

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
                &format!("{SHORTCUT_MODIFIER}-s"),
                SaveDocument,
                Some(context),
            ),
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
    store: Option<DocumentStore>,
    history: Vec<String>,
    current_name: Option<String>,
    saved_source: String,
    pending_document: Option<DocumentDestination>,
    rename_input: Entity<InputState>,
    renaming: bool,
}

impl JsonEditor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_store(window, cx, DocumentStore::in_user_directory())
    }

    fn with_store(
        window: &mut Window,
        cx: &mut Context<Self>,
        store: io::Result<DocumentStore>,
    ) -> Self {
        let (store, history, status, error) = match store {
            Ok(store) => match store.list() {
                Ok(history) => (
                    Some(store),
                    history,
                    "新文档 · 输入内容后保存".into(),
                    false,
                ),
                Err(error) => (Some(store), vec![], format!("读取历史失败：{error}"), true),
            },
            Err(error) => (None, vec![], format!("创建数据目录失败：{error}"), true),
        };
        let rename_input = cx.new(|cx| InputState::new(window, cx));
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
                .folding(true)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .default_value("")
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
            status,
            error,
            _subscriptions: vec![subscription],
            operation_source: None,
            store,
            history,
            current_name: None,
            saved_source: String::new(),
            pending_document: None,
            rename_input,
            renaming: false,
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

    fn save(&mut self, _: &SaveDocument, window: &mut Window, cx: &mut Context<Self>) {
        if self.save_current(cx) {
            if let Some(destination) = self.pending_document.take() {
                self.open_document(destination, window, cx);
            }
        }
    }

    fn save_current(&mut self, cx: &mut Context<Self>) -> bool {
        let source = self.editor.read(cx).value().to_string();
        let result = match self.store.as_ref() {
            Some(store) => match self.current_name.as_ref() {
                Some(name) => store.save(name, &source).map(|()| name.clone()),
                None => store.create(&source),
            },
            None => Err(io::Error::other(
                "数据目录不可用，请检查用户目录权限后重新启动",
            )),
        };
        match result {
            Ok(name) => {
                self.current_name = Some(name.clone());
                self.saved_source = source;
                self.status = format!("已保存 · {name}");
                self.error = false;
                self.refresh_history(cx);
                cx.notify();
                true
            }
            Err(error) => {
                self.status = format!("保存失败：{error} · 内容仍保留在编辑器中");
                self.error = true;
                cx.notify();
                false
            }
        }
    }

    fn refresh_history(&mut self, cx: &mut Context<Self>) {
        if let Some(store) = self.store.as_ref() {
            match store.list() {
                Ok(history) => self.history = history,
                Err(error) => {
                    self.status = format!("刷新历史失败：{error}");
                    self.error = true;
                }
            }
        }
        cx.notify();
    }

    fn request_document(
        &mut self,
        destination: DocumentDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let DocumentDestination::History(name) = &destination {
            if self.current_name.as_ref() == Some(name) {
                return;
            }
        }
        if self.editor.read(cx).value().as_str() != self.saved_source {
            self.pending_document = Some(destination);
            self.status = "当前文档有未保存改动 · 请选择保存、放弃或取消".into();
            self.error = false;
            cx.notify();
        } else {
            self.open_document(destination, window, cx);
        }
    }

    fn open_document(
        &mut self,
        destination: DocumentDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = match destination {
            DocumentDestination::New => Ok((None, String::new())),
            DocumentDestination::History(name) => self
                .store
                .as_ref()
                .ok_or_else(|| io::Error::other("数据目录不可用"))
                .and_then(|store| store.read(&name))
                .map(|source| (Some(name), source)),
        };
        match result {
            Ok((name, source)) => {
                self.status = match &name {
                    Some(name) => format!("已打开 · {name}"),
                    None => "新文档 · 输入内容后保存".into(),
                };
                self.error = false;
                self.current_name = name;
                self.saved_source = source.clone();
                self.operation_source = Some(source.clone());
                self.renaming = false;
                self.pending_document = None;
                self.editor.update(cx, |state, cx| {
                    // Loading another document starts a fresh undo history.
                    state.set_value(source, window, cx);
                    state.focus(window, cx);
                });
            }
            Err(error) => {
                self.status = format!("打开失败：{error} · 当前文档未修改");
                self.error = true;
            }
        }
        cx.notify();
    }

    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(name) = self.current_name.clone() {
            self.rename_input.update(cx, |state, cx| {
                state.set_value(name, window, cx);
                state.focus(window, cx);
            });
            self.renaming = true;
            cx.notify();
        }
    }

    fn rename_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(old_name) = self.current_name.clone() else {
            return;
        };
        let requested = self.rename_input.read(cx).value().to_string();
        let result = self
            .store
            .as_ref()
            .ok_or_else(|| io::Error::other("数据目录不可用"))
            .and_then(|store| store.rename(&old_name, &requested));
        match result {
            Ok(name) => {
                self.current_name = Some(name.clone());
                self.renaming = false;
                self.status = format!("已重命名 · {name}");
                self.error = false;
                self.refresh_history(cx);
                self.editor.update(cx, |state, cx| state.focus(window, cx));
            }
            Err(error) => {
                self.status = format!("重命名失败：{error}");
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
            .on_action(cx.listener(Self::save))
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .gap_3()
                    .flex_wrap()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().text_lg().font_semibold().child("JSON Editor"))
                    .child(div().flex_1())
                    .child(
                        Button::new("save-document")
                            .label("保存")
                            .tooltip_with_action("保存当前文档", &SaveDocument, Some("JsonEditor"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save(&SaveDocument, window, cx)
                            })),
                    )
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
                    .child(format!(
                        "{}{}",
                        self.current_name.as_deref().unwrap_or("未命名文档"),
                        if source.as_str() != self.saved_source { " · 未保存" } else { "" }
                    ))
                    .child(format!(
                        "{SHORTCUT_LABEL}+S 保存 · {SHORTCUT_LABEL}+Shift+F 格式化 · {SHORTCUT_LABEL}+Enter 验证"
                    )),
            )
            .when(self.pending_document.is_some(), |this| {
                this.child(
                    h_flex()
                        .px_6().py_2().gap_2().flex_wrap()
                        .child("当前文档尚未保存")
                        .child(Button::new("save-and-switch").primary().label("保存并继续")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save(&SaveDocument, window, cx)
                            })))
                        .child(Button::new("discard-and-switch").label("放弃改动并继续")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(destination) = this.pending_document.take() {
                                    this.open_document(destination, window, cx);
                                }
                            })))
                        .child(Button::new("cancel-switch").label("取消")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pending_document = None;
                                this.status = "已取消切换 · 当前文档未修改".into();
                                this.error = false;
                                this.editor.update(cx, |state, cx| state.focus(window, cx));
                                cx.notify();
                            })))
                )
            })
            .when(self.renaming, |this| {
                this.child(
                    h_flex().px_6().py_2().gap_2()
                        .child(div().flex_1().child(Input::new(&self.rename_input).id("rename-input")))
                        .child(Button::new("confirm-rename").primary().label("确认重命名")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.rename_document(window, cx)
                            })))
                        .child(Button::new("cancel-rename").label("取消")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.renaming = false;
                                this.editor.update(cx, |state, cx| state.focus(window, cx));
                                cx.notify();
                            })))
                )
            })
            .child(
                h_flex().flex_1().min_h_0().items_stretch()
                    .child(
                        div().w(px(220.)).flex_shrink_0().v_flex().gap_2().p_3()
                            .border_r_1().border_color(cx.theme().border)
                            .child(h_flex().gap_2()
                                .child(div().flex_1().font_semibold().child("文档历史"))
                                .child(Button::new("refresh-history").label("刷新")
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh_history(cx)))))
                            .child(Button::new("new-document").label("新建文档")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.request_document(DocumentDestination::New, window, cx)
                                })))
                            .child(Button::new("rename-document").label("重命名当前文档")
                                .disabled(self.current_name.is_none())
                                .on_click(cx.listener(|this, _, window, cx| this.start_rename(window, cx))))
                            .child(
                                div().id("document-history").flex_1().min_h_0().overflow_y_scroll()
                                    .v_flex().gap_1()
                                    .when(self.history.is_empty(), |this| {
                                        this.child(div().text_sm().text_color(cx.theme().muted_foreground)
                                            .child("暂无历史 · 保存后显示"))
                                    })
                                    .children(self.history.iter().enumerate().map(|(index, name)| {
                                        let name = name.clone();
                                        Button::new(("history-document", index))
                                            .label(name.clone())
                                            .tooltip(name.clone())
                                            .w_full()
                                            .when(self.current_name.as_ref() == Some(&name), |button| button.primary())
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.request_document(DocumentDestination::History(name.clone()), window, cx)
                                            }))
                                    })),
                            )
                            .child(div().text_xs().text_color(cx.theme().muted_foreground)
                                .child(self.store.as_ref().map(|store| store.directory().display().to_string())
                                    .unwrap_or_else(|| "数据目录不可用".into()))),
                    )
                    .child(
                        div().flex_1().min_w_0().min_h_0().px_4().pb_3().child(
                            Editor::new(&self.editor)
                                .h_full().w_full().bordered(false).text_size(px(15.)),
                        ),
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
    use super::{
        DocumentDestination, DocumentStore, FormatJson, JsonEditor, SHORTCUT_MODIFIER,
        SaveDocument, ValidateJson, bind_shortcuts,
    };
    use gpui_kit::{
        AppContext, ClipboardItem, TestAppContext, WindowOptions, component::input::Undo,
        test::TestWindowExt,
    };

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self(
                std::env::temp_dir()
                    .join(format!("json-editor-ui-{}-{unique}", std::process::id())),
            )
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[gpui_kit::test]
    fn saves_opens_renames_and_protects_unsaved_documents(cx: &mut TestAppContext) {
        let directory = TestDirectory::new();
        cx.update(|cx| {
            gpui_kit::init(cx);
            bind_shortcuts(cx);
        });
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    JsonEditor::with_store(window, cx, DocumentStore::new(directory.0.clone()))
                })
            })
            .unwrap()
        });
        let source = "{\n  \"历史\": true\n}";
        cx.update_window(window, |_, window, cx| {
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), "");
            assert!(view.read(cx).current_name.is_none());
            view.read(cx).editor.clone().update(cx, |state, cx| {
                state.set_value(source, window, cx);
                state.focus(window, cx);
            });
            window.render_frame(cx);
        })
        .unwrap();
        cx.simulate_keystrokes(window.into(), &format!("{SHORTCUT_MODIFIER}-s"));
        let name = cx.update(|cx| view.read(cx).current_name.clone().unwrap());
        assert_eq!(
            std::fs::read_to_string(directory.0.join(&name)).unwrap(),
            source
        );
        cx.update_window(window, |_, window, cx| {
            assert_eq!(view.read(cx).history.len(), 1);
            assert_eq!(view.read(cx).saved_source, source);
            view.read(cx).editor.clone().update(cx, |state, cx| {
                state.set_value("{draft", window, cx);
            });
            view.update(cx, |this, cx| {
                this.request_document(DocumentDestination::New, window, cx);
            });
            assert!(view.read(cx).pending_document.is_some());
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), "{draft");
            window.render_frame(cx);
            window.click("cancel-switch", cx);
            assert!(view.read(cx).pending_document.is_none());
            view.update(cx, |this, cx| {
                this.request_document(DocumentDestination::New, window, cx);
            });
            window.render_frame(cx);
            window.click("save-and-switch", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(directory.0.join(&name)).unwrap(),
            "{draft"
        );
        cx.update_window(window, |_, window, cx| {
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), "");
            assert!(view.read(cx).current_name.is_none());
            view.update(cx, |this, cx| {
                this.request_document(DocumentDestination::History(name.clone()), window, cx);
            });
            assert_eq!(view.read(cx).editor.read(cx).value().as_str(), "{draft");
            view.read(cx).editor.clone().update(cx, |state, cx| {
                state.set_value("unsaved changes", window, cx);
            });
            view.update(cx, |this, cx| this.start_rename(window, cx));
            view.read(cx).rename_input.clone().update(cx, |state, cx| {
                state.set_value("工作文档", window, cx);
            });
            view.update(cx, |this, cx| this.rename_document(window, cx));
            assert_eq!(view.read(cx).current_name.as_deref(), Some("工作文档.json"));
            assert_eq!(
                view.read(cx).editor.read(cx).value().as_str(),
                "unsaved changes"
            );
            view.update(cx, |this, cx| this.save(&SaveDocument, window, cx));
            assert_eq!(view.read(cx).history, vec!["工作文档.json"]);
        })
        .unwrap();
        assert!(!directory.0.join(&name).exists());
        assert_eq!(
            std::fs::read_to_string(directory.0.join("工作文档.json")).unwrap(),
            "unsaved changes"
        );
    }

    #[gpui_kit::test]
    fn failed_save_does_not_switch_or_discard_edits(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    JsonEditor::with_store(window, cx, Err(std::io::Error::other("unavailable")))
                })
            })
            .unwrap()
        });
        cx.update_window(window, |_, window, cx| {
            view.read(cx).editor.clone().update(cx, |state, cx| {
                state.set_value("keep my edits", window, cx);
            });
            view.update(cx, |this, cx| {
                this.request_document(DocumentDestination::New, window, cx);
                this.save(&SaveDocument, window, cx);
            });
            assert!(view.read(cx).error);
            assert!(view.read(cx).pending_document.is_some());
            assert_eq!(
                view.read(cx).editor.read(cx).value().as_str(),
                "keep my edits"
            );
            assert!(view.read(cx).current_name.is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn clipboard_buttons_shortcuts_and_undo(cx: &mut TestAppContext) {
        let directory = TestDirectory::new();
        cx.update(|cx| {
            gpui_kit::init(cx);
            bind_shortcuts(cx);
        });
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    JsonEditor::with_store(window, cx, DocumentStore::new(directory.0.clone()))
                })
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
            assert_eq!(
                view.read(cx).editor.read(cx).value().as_str(),
                "{invalid JSON"
            );
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().unwrap(),
                "{invalid JSON"
            );
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
        let directory = TestDirectory::new();
        cx.update(gpui_kit::init);
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    JsonEditor::with_store(window, cx, DocumentStore::new(directory.0.clone()))
                })
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
