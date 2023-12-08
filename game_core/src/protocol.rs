use crate::types::{Coords, Map, Block};
use proto_dryb::{Serialize, SerializeError, Deserialize, DeserializeError};
use proto_dryb_derive::{Serialize, Deserialize};

use std::cmp::{max, min};

#[derive(Serialize, Deserialize)]
pub enum Packet {
    Server(ServerPacket),
}

#[derive(Serialize, Deserialize)]
pub enum ServerPacket {
    NewClientCoordsVisibleMap(NewClient),
}

#[derive(Serialize, Deserialize)]
pub struct NewClient {
    pub coords: Coords,
    pub map: Vec<(Block, Coords)>,
}

pub fn generate_initial_payload(
    buf: &mut [u8],
    coords: Coords,
    map: &Map,
    ) -> Result<usize, SerializeError> {
    let packet = Packet::Server(ServerPacket::NewClientCoordsVisibleMap(NewClient::new(coords, map, 5)));

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
    // TODO dont store constants ROWS COLLS buit create a MAP struct with everything configured
    let bottom_right = (
        min(1 + coords.0 + radius as i16, map.height as i16),
        min(1 + coords.1 + radius as i16, map.width as i16),
    );

    assert!(top_left.0 <= bottom_right.0);
    assert!(top_left.1 <= bottom_right.1);

    let mut res = vec![];
    for i in top_left.0..bottom_right.0 {
        for j in top_left.1..bottom_right.1 {
            let a = coords.0 - i;
            let b = coords.1 - j;

            if a * a + b * b <= radius_square {
                res.push((map.coords[i as usize][j as usize], (i, j)));
            }
        }
    }

    res
}
