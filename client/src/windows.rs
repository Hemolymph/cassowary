mod game;
mod menu;

pub use menu::Message as MenuMessage;
pub use menu::State as MenuState;
pub use menu::view as menu_view;

pub use game::Message as GameMessage;
pub use game::State as GameState;
pub use game::view as game_view;

pub enum Window {
    Menu(MenuState),
    Game(GameState),
}

impl Default for Window {
    fn default() -> Self {
        Self::Menu(MenuState::default())
    }
}
