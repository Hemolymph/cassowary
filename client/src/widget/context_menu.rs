mod c_overlay;
use c_overlay::ContextOverlay;
use iced::{
    Point, Size,
    advanced::{
        Widget,
        graphics::core::{Element, widget},
        overlay,
        widget::{Tree, tree},
    },
};

pub struct ContextMenu<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    pub content: Element<'a, Message, Theme, Renderer>,
    pub context_menu: Box<dyn Fn() -> Element<'a, Message, Theme, Renderer>>,
}

#[derive(Default)]
struct State {
    displaying: bool,
    cursor_position: Point,
}

impl<Message, Theme, Renderer: iced::advanced::Renderer> Widget<Message, Theme, Renderer>
    for ContextMenu<'_, Message, Theme, Renderer>
{
    fn children(&self) -> Vec<tree::Tree> {
        vec![
            widget::Tree::new(&self.content),
            widget::Tree::new((self.context_menu)()),
        ]
    }
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> iced::advanced::widget::tree::State {
        tree::State::new(State::default())
    }
    fn size(&self) -> iced::Size<iced::Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<iced::Length> {
        self.content.as_widget().size_hint()
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content, &(self.context_menu)()]);
    }

    fn layout(
        &self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.content
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        // if !cursor.is_over(layout.bounds()) {
        //     state.displaying = false;
        // }

        if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Right)) =
            event
        {
            let bounds = layout.bounds();

            if cursor.is_over(bounds) {
                state.displaying = true;
                state.cursor_position = cursor.position().unwrap();

                shell.request_redraw();
                shell.capture_event();
            }
        }
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: iced::advanced::Layout<'a>,
        renderer: &Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'a, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        if state.displaying {
            let overlay = (self.context_menu)();
            overlay.as_widget().diff(&mut tree.children[1]);
            let a =
                ContextOverlay::new(state.cursor_position, &mut tree.children[1], overlay, state);
            return Some(overlay::Element::new(Box::new(a)));
        }

        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
