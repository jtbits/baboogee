use rand::{thread_rng, Rng};

use crate::types::{Block, Coords, Map};

pub fn generate_random_coords(max_x: usize, max_y: usize) -> Coords {
    let mut rng = thread_rng();

    (
        rng.gen_range(0..max_x) as u16,
        rng.gen_range(0..max_y) as u16,
    )
}

pub fn is_inside_circle(
    (center_x, center_y): Coords,
    radius: u8,
    (other_x, other_y): Coords,
) -> bool {
    let diff_sqr =
        (center_x as i16 - other_x as i16).pow(2) + (center_y as i16 - other_y as i16).pow(2);
    let radius_sqr = (radius as i16).pow(2);

    diff_sqr <= radius_sqr
}

pub fn generate_map() -> Map {
    let mut rng = rand::thread_rng();
    let height = rng.gen_range(20..50);
    let width = rng.gen_range(20..50);
    let coords = vec![vec![Block::Grass; width]; height];

    Map {
        height,
        width,
        coords,
    }
}
