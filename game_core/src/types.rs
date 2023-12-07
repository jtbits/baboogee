pub type Coords = (i16, i16);

#[derive(Clone, Copy)]
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

pub struct Map {
    pub height: u16,
    pub width: u16,
    pub coords: Vec<Vec<Block>>,
}
