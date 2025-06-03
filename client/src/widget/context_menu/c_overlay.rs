use iced::{
    Element, Point, Size,
    advanced::{Overlay, layout, widget::Tree},
    keyboard::Key,
};

use super::{ContextMenu, State};

pub struct ContextOverlay<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    // The position of the element
    pub position: Point,
    /// The state of the [`ContextMenuOverlay`].
    pub tree: &'a mut Tree,
    /// The content of the [`ContextMenuOverlay`].
    pub content: Element<'a, Message, Theme, Renderer>,
    /// The state shared between [`ContextMenu`](crate::widget::ContextMenu) and [`ContextMenuOverlay`].
    pub state: &'a mut State,
}

impl<Message, Theme, Renderer: iced::advanced::Renderer> Overlay<Message, Theme, Renderer>
    for ContextOverlay<'_, Message, Theme, Renderer>
{
    fn layout(&mut self, renderer: &Renderer, bounds: iced::Size) -> iced::advanced::layout::Node {
        // Try to stay inside borders
        let content = self.content.as_widget().layout(
            self.tree,
            renderer,
            &layout::Limits::new(Size::ZERO, bounds),
        );

        let mut position = self.position;
        if position.x + content.bounds().size().width > bounds.width {
            position.x = f32::max(0.0, position.x - content.size().width);
        }
        if position.y + content.size().height > bounds.height {
            position.y = f32::max(0.0, position.y - content.size().height);
        }

        content.move_to(position)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
    ) {
        let bounds = layout.bounds();

        // Background
        // if (bounds.width > 0.) && (bounds.height > 0.) {
        //     renderer.fill_quad(
        //         renderer::Quad {
        //             bounds,
        //             border: Border {
        //                 radius: (0.0).into(),
        //                 width: 0.0,
        //                 color: Color::TRANSPARENT,
        //             },
        //             shadow: Shadow::default(),
        //             snap: true,
        //         },
        //         Color::WHITE,
        //     );
        // }

        // let content_layout = layout
        //     .children()
        //     .next()
        //     .expect("widget: Layout should have a content layout.");

        // Modal
        self.content
            .as_widget()
            .draw(self.tree, renderer, theme, style, layout, cursor, &bounds);
    }

    fn update(
        &mut self,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
    ) {
        self.content.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );

        if shell.is_event_captured() {
            return;
        }

        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                if *key == Key::Named(iced::keyboard::key::Named::Escape) {
                    self.state.displaying = false;
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                if !cursor.is_over(layout.bounds()) {
                    self.state.displaying = false;
                    shell.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl<'a, Message, Theme, Renderer> ContextOverlay<'a, Message, Theme, Renderer> {
    /// Creates a new [`ContextMenuOverlay`].
    pub(crate) fn new<C>(
        position: Point,
        tree: &'a mut Tree,
        content: C,
        state: &'a mut State,
    ) -> Self
    where
        C: Into<Element<'a, Message, Theme, Renderer>>,
    {
        Self {
            position,
            tree,
            content: content.into(),
            state,
        }
    }
}

impl<'a, Message: Clone + 'a, Theme: 'a, Renderer: iced::advanced::Renderer + 'a>
    From<ContextMenu<'a, Message, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
{
    fn from(value: ContextMenu<'a, Message, Theme, Renderer>) -> Self {
        Self::new(value)
    }
}
