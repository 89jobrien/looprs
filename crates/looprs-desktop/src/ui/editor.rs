use freya::code_editor::*;
use freya::prelude::*;

const DEFAULT_EDITOR_TEXT: &str = r#"// looprs desktop editor

fn main() {
    println!(\"Edit me with freya-code-editor\");
}
"#;

pub fn editor_screen() -> Element {
    Element::from(EditorScreen)
}

#[derive(Clone, Copy, PartialEq)]
pub struct EditorScreen;

impl Component for EditorScreen {
    fn render(&self) -> impl IntoElement {
        use_init_theme(|| DARK_THEME);
        let focus = use_focus();

        let editor = use_state(|| {
            let rope = Rope::from_str(DEFAULT_EDITOR_TEXT);
            let mut editor = CodeEditorData::new(rope, LanguageId::Rust);
            editor.parse();
            editor.measure(14.0);
            editor
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .vertical()
            .spacing(8.0)
            .child(label().text("Scratchpad"))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .background((28, 28, 28))
                    .border(Border::new().fill((60, 60, 60)).width(1.0))
                    .corner_radius(8.0)
                    .padding(Gaps::new_all(10.0))
                    .child(
                        CodeEditor::new(editor, focus.a11y_id())
                            .font_size(14.0)
                            .line_height(1.4)
                            .show_whitespace(false),
                    ),
            )
    }
}
