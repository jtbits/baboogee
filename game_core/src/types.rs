use proto_dryb::*;
use proto_dryb_derive::{Deserialize, Serialize};

pub type Coords = (i16, i16);

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

#[derive(Default)]
pub struct Map {
    pub height: u16,
    pub width: u16,
    pub coords: Vec<Vec<Block>>,
}
