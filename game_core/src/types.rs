use proto_dryb::*;
use proto_dryb_derive::{Deserialize, Serialize};

pub type Coords = (i16, i16);

pub type MoveCoords = (Coords, Vec<(Block, Coords)>);

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum Block {
    Void,
    Grass,
    Player,
    OtherPlayer,

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
