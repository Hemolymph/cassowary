use context_menu::ContextMenu;
use iced::advanced::graphics::core::Element;

pub mod context_menu;

pub fn context_menu<'a, Message, Theme, Renderer>(
    underlay: impl Into<Element<'a, Message, Theme, Renderer>>,
    overlay: impl Fn() -> Element<'a, Message, Theme, Renderer> + 'static,
) -> ContextMenu<'a, Message, Theme, Renderer> {
    ContextMenu {
        content: underlay.into(),
        context_menu: Box::new(overlay),
    }
}
