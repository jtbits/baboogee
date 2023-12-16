use crate::types::{Coords, MapCell};
use proto_dryb::{Deserialize, DeserializeError, Serialize, SerializeError};
use proto_dryb_derive::{Deserialize, Serialize};

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
    PlayerWasShot(u8, Direction),
    PlayerDied(u32),
}

pub fn generate_player_died_payload(buf: &mut [u8], by_id: u32) -> Result<usize, SerializeError> {
    Packet::Server(ServerPacket::PlayerDied(by_id)).serialize(buf)
}

pub fn generate_shoot_payload(
    buf: &mut [u8],
    damage: u8,
    direction: Direction,
) -> Result<usize, SerializeError> {
    Packet::Server(ServerPacket::PlayerWasShot(damage, direction)).serialize(buf)
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
    pub radius: u8,
    pub weapon_range: u8,
    pub visible_coords: Vec<MapCell>,
    pub players: Vec<Player>,
}

impl NewClient {
    fn new(
        id: u32,
        coords: Coords,
        visible_coords: Vec<MapCell>,
        radius: u8,
        hp: u8,
        weapon_range: u8,
        players: Vec<Player>,
    ) -> Self {
        Self {
            id,
            coords,
            hp,
            radius,
            weapon_range,
            visible_coords,
            players,
        }
    }
}

pub fn generate_initial_payload(
    buf: &mut [u8],
    id: u32,
    coords: Coords,
    radius: u8,
    hp: u8,
    weapon_range: u8,
    visible_coords: Vec<MapCell>,
    players: Vec<Player>,
) -> Result<usize, SerializeError> {
    let packet = Packet::Server(ServerPacket::NewClientCoordsVisibleMap(NewClient::new(
        id,
        coords,
        visible_coords,
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
    fn new(center: Coords, coords: Vec<MapCell>, players: Vec<Player>) -> Self {
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
