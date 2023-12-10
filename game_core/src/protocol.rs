use crate::types::{Block, Coords, Map};
use proto_dryb::{Deserialize, DeserializeError, Serialize, SerializeError};
use proto_dryb_derive::{Deserialize, Serialize};

use std::cmp::{max, min};

#[derive(Serialize, Deserialize)]
pub enum Packet {
    Server(ServerPacket),
    Client(ClientPacket),
}

#[derive(Serialize, Deserialize)]
pub enum ServerPacket {
    NewClientCoordsVisibleMap(NewClient),
    NewCoords(NewCoords),
    OtherPlayerMoved(OtherPlayerMoved),
}

#[derive(Serialize, Deserialize)]
pub struct OtherPlayerMoved {
    pub coords: Coords,
    pub id: u32,
}

pub fn generate_move_notify_payload(
    buf: &mut [u8],
    coords: Coords,
    id: u32,
) -> Result<usize, SerializeError> {
    let opm = OtherPlayerMoved { coords, id };
    let packet = Packet::Server(ServerPacket::OtherPlayerMoved(opm));

    packet.serialize(buf)
}

#[derive(Serialize, Deserialize)]
pub enum ClientPacket {
    Move(Step),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Step {
    Up,
    Right,
    Down,
    Left,
}

impl TryFrom<char> for Step {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'w' | 'k' => Ok(Self::Up),
            'd' | 'l' => Ok(Self::Right),
            's' | 'j' => Ok(Self::Down),
            'a' | 'h' => Ok(Self::Left),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct NewClient {
    pub coords: Coords,
    pub map: Vec<(Block, Coords)>,
}

pub fn generate_initial_payload(
    buf: &mut [u8],
    coords: Coords,
    radius: u8,
    map: &Map,
) -> Result<usize, SerializeError> {
    let packet = Packet::Server(ServerPacket::NewClientCoordsVisibleMap(NewClient::new(
        coords, map, radius,
    )));

    packet.serialize(buf)
}

#[derive(Serialize, Deserialize)]
pub struct NewCoords {
    pub center: Coords,
    pub coords: Vec<(Block, Coords)>,
}

impl NewCoords {
    fn new(center: (i16, i16), coords: Vec<(Block, Coords)>) -> Self {
        Self { center, coords }
    }
}

pub fn generate_new_coords_payload(
    buf: &mut [u8],
    new_player_coord: Coords,
    new_visiple_coord: Vec<(Block, Coords)>,
) -> Result<usize, SerializeError> {
    let packet = Packet::Server(ServerPacket::NewCoords(NewCoords::new(
        new_player_coord,
        new_visiple_coord,
    )));

    packet.serialize(buf)
}

impl NewClient {
    fn new(coords: Coords, map: &Map, radius: u8) -> Self {
        Self {
            coords,
            map: visible_map(map, coords, radius),
        }
    }
}

fn visible_map(map: &Map, coords: Coords, radius: u8) -> Vec<(Block, Coords)> {
    let radius_square = radius as i16 * radius as i16;

    let top_left = (
        max(coords.0 - radius as i16, 0),
        max(coords.1 - radius as i16, 0),
    );
    let bottom_right = (
        min(1 + coords.0 + radius as i16, map.height as i16),
        min(1 + coords.1 + radius as i16, map.width as i16),
    );

    assert!(top_left.0 <= bottom_right.0);
    assert!(top_left.1 <= bottom_right.1);

    let mut res = vec![];
    for i in top_left.0..bottom_right.0 {
        for j in top_left.1..bottom_right.1 {
            let x = coords.0 - i;
            let y = coords.1 - j;

            // draw circle
            if x.pow(2) + y.pow(2) <= radius_square {
                res.push((map.coords[i as usize][j as usize], (i, j)));
            }
        }
    }

    res
}
