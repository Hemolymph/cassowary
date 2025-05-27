use macroquad::{
    color::{Color, RED},
    input::{is_mouse_button_pressed, mouse_position},
    math::Vec2,
    shapes::draw_rectangle,
    texture::{DrawTextureParams, draw_texture_ex},
};
use shared::PlaceFrom;

use crate::{Image, TEXTURES};

struct Holding {
    card: Card,
    from: PlaceFrom,
}

pub trait Draw {
    async fn draw(&self);
    fn contains(&self, point: Vec2) -> bool;
}

pub trait Click: Draw {
    fn is_clicked(&self) -> bool {
        is_mouse_button_pressed(macroquad::input::MouseButton::Left)
            && self.contains(mouse_position().into())
    }
}

pub struct Clickable<T: Draw> {
    drawable: T,
}

impl<T: Draw> Draw for Clickable<T> {
    async fn draw(&self) {
        self.drawable.draw().await
    }

    fn contains(&self, point: Vec2) -> bool {
        self.drawable.contains(point)
    }
}

impl<T: Draw> Click for Clickable<T> {}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
}

impl Rect {
    pub const fn clickable(self) -> Clickable<Self> {
        Clickable { drawable: self }
    }
}

impl Draw for Rect {
    async fn draw(&self) {
        draw_rectangle(self.x, self.y, self.w, self.h, self.color);
    }

    fn contains(&self, point: Vec2) -> bool {
        Into::<macroquad::math::Rect>::into(self.clone()).contains(point)
    }
}

impl From<Rect> for macroquad::math::Rect {
    fn from(value: Rect) -> Self {
        Self {
            x: value.x,
            y: value.y,
            w: value.w,
            h: value.h,
        }
    }
}

pub struct Card {
    pub image: String,
    pub rect: Rect,
}

impl Draw for Card {
    async fn draw(&self) {
        let textures = TEXTURES.read();
        draw_texture_ex(
            textures
                .get(&Image::Name(self.image.clone()))
                .unwrap_or(textures.get(&Image::CardBack).unwrap()),
            self.rect.x,
            self.rect.y,
            RED,
            DrawTextureParams {
                dest_size: Some(Vec2::new(self.rect.w, self.rect.y)),
                source: None,
                rotation: 0.,
                flip_x: false,
                flip_y: false,
                pivot: None,
            },
        );
    }

    fn contains(&self, point: Vec2) -> bool {
        self.rect.contains(point)
    }
}

impl Click for Card {}
