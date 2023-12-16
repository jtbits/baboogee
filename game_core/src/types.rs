use proto_dryb::*;
use proto_dryb_derive::{Deserialize, Serialize};

pub type Coords = (u16, u16);

#[derive(Serialize, Deserialize)]
pub struct MapCell {
    pub block: Block,
    pub coords: Coords,
}

impl MapCell {
    pub fn new(block: Block, coords: Coords) -> Self {
        Self { block, coords }
    }
}

pub struct Map {
    pub height: usize,
    pub width: usize,
    pub coords: Vec<Vec<Block>>,
}

pub type MoveCoords = (Coords, Vec<MapCell>);

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
