use proto_dryb_derive::{Serialize, Deserialize};
use proto_dryb::*;

pub type Coords = (i16, i16);

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Block {
    Void,
    Grass,
    Player,

    WallHorizontal,
    WallVertical,
    WallTopLeft,
    WallTopRight,
    WallBottomLeft,
    WallBottomRight,
}

#[derive(Default)]
pub struct Map {
    pub height: u16,
    pub width: u16,
    pub coords: Vec<Vec<Block>>,
}
