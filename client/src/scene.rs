mod game;
mod lobby;
pub use game::GameData;
use game::draw_game;
pub use lobby::LobbyData;
use lobby::draw_lobby_select;
use shared::ClientMsg;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum Scene {
    LobbySelect(LobbyData),
    Game(GameData),
}

impl Scene {
    pub async fn draw(&mut self, to_server: &UnboundedSender<ClientMsg>) {
        match self {
            Scene::LobbySelect(data) => {
                if let Some(a) = draw_lobby_select(to_server, data) {
                    *self = a
                }
            }
            Scene::Game(data) => draw_game(to_server, data).await,
        }
    }
}
