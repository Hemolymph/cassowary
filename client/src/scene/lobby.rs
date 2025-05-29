use egui_macroquad::egui;
use shared::ClientMsg;
use tokio::sync::mpsc::UnboundedSender;

use super::Scene;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LobbyData {
    pub room: String,
}

pub fn draw_lobby_select(
    to_server: &UnboundedSender<ClientMsg>,
    scene: &mut LobbyData,
) -> Option<Scene> {
    let next_scene = None;
    egui_macroquad::ui(|ctx| {
        // Get the screen size from egui
        let screen_rect = ctx.screen_rect();
        let window_size = egui::vec2(300.0, 100.0);

        // Calculate center position
        let center_pos = screen_rect.center() - window_size / 2.0;
        let win = egui::Window::new("Cassowary")
            .resizable(true)
            .collapsible(false)
            .movable(false)
            .resizable(false)
            .current_pos(center_pos);

        win.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut scene.room);
                    let join_room = ui.button("Join Room");
                    if join_room.clicked() {
                        to_server
                            .send(ClientMsg::JoinRoom(scene.room.clone()))
                            .unwrap();
                    }
                });
                let create_room = ui.button("Create Room");

                if create_room.clicked() {
                    to_server
                        .send(ClientMsg::CreateRoom(scene.room.clone()))
                        .unwrap();
                }
            })
        });
    });

    next_scene
}
