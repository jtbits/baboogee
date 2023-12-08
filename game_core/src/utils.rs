use rand::{thread_rng, Rng};

use crate::types::{Coords, Map, Block};

pub fn generate_random_coords(max_x: i16, max_y: i16) -> Coords {
    let mut rng = thread_rng();

    (rng.gen_range(0..max_x), rng.gen_range(0..max_y))
}

pub fn generate_map() -> Map {
    let mut rng = rand::thread_rng();
    let height = rng.gen_range(100..300);
    let width = rng.gen_range(100..300);
    let coords = vec![vec![Block::Grass; width as usize]; height as usize];

    Map {
        height,
        width,
        coords, 
    }
}
