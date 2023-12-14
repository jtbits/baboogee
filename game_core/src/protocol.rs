use crate::types::{Coords, Map, MapCell};
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
    OtherPlayerMovedOutsideRadius(u32),
    PlayerDisconnected(u32),
}

pub fn generate_player_disconnected(buf: &mut [u8], id: u32) -> Result<usize, SerializeError> {
    Packet::Server(ServerPacket::PlayerDisconnected(id)).serialize(buf)
}

#[derive(Serialize, Deserialize)]
pub struct OtherPlayerMovedOutsideRadius {
    pub id: u32,
}

pub fn generate_move_outside_radius_notify_payload(
    buf: &mut [u8],
    id: u32,
) -> Result<usize, SerializeError> {
    let packet = Packet::Server(ServerPacket::OtherPlayerMovedOutsideRadius(id));

    packet.serialize(buf)
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
    Move(Direction),
    Shoot(Direction),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

impl TryFrom<char> for Direction {
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
pub struct Player {
    pub id: u32,
    pub coords: Coords,
}

impl Player {
    pub fn new(id: u32, coords: Coords) -> Self {
        Self { id, coords }
    }
}

#[derive(Serialize, Deserialize)]
pub struct NewClient {
    pub id: u32,
    pub coords: Coords,
    pub hp: u8,
    pub weapon_range: u8,
    pub map: Vec<MapCell>,
    pub players: Vec<Player>,
}

impl NewClient {
    fn new(
        id: u32,
        coords: Coords,
        map: &Map,
        radius: u8,
        hp: u8,
        weapon_range: u8,
        players: Vec<Player>,
    ) -> Self {
        Self {
            id,
            coords,
            players,
            hp,
            weapon_range,
            map: visible_map(map, coords, radius),
        }
    }
}

fn visible_map(map: &Map, coords: Coords, radius: u8) -> Vec<MapCell> {
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
                res.push(MapCell {
                    block: map.coords[i as usize][j as usize],
                    coords: (i, j),
                });
            }
        }
    }

    res
}
pub fn generate_initial_payload(
    buf: &mut [u8],
    id: u32,
    coords: Coords,
    radius: u8,
    hp: u8,
    weapon_range: u8,
    map: &Map,
    players: Vec<Player>,
) -> Result<usize, SerializeError> {
    let packet = Packet::Server(ServerPacket::NewClientCoordsVisibleMap(NewClient::new(
        id,
        coords,
        map,
        radius,
        hp,
        weapon_range,
        players,
    )));

    packet.serialize(buf)
}

#[derive(Serialize, Deserialize)]
pub struct NewCoords {
    pub center: Coords,
    pub coords: Vec<MapCell>,
    pub players: Vec<Player>,
}

impl NewCoords {
    fn new(center: (i16, i16), coords: Vec<MapCell>, players: Vec<Player>) -> Self {
        Self {
            center,
            coords,
            players,
        }
    }
}

pub fn generate_new_coords_payload(
    buf: &mut [u8],
    new_player_coord: Coords,
    new_visiple_coord: Vec<MapCell>,
    visible_players: Vec<Player>,
) -> Result<usize, SerializeError> {
    let packet = Packet::Server(ServerPacket::NewCoords(NewCoords::new(
        new_player_coord,
        new_visiple_coord,
        visible_players,
    )));

    packet.serialize(buf)
}
