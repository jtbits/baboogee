use std::cmp::{max, min};

use rand::{thread_rng, Rng};

use crate::{
    protocol::Step,
    types::{Block, Coords, Map, MapCell, MoveCoords},
};

pub fn generate_random_coords(max_x: i16, max_y: i16) -> Coords {
    let mut rng = thread_rng();

    (rng.gen_range(0..max_x), rng.gen_range(0..max_y))
}

pub fn generate_map() -> Map {
    let mut rng = rand::thread_rng();
    let height = rng.gen_range(20..50);
    let width = rng.gen_range(20..50);
    let coords = vec![vec![Block::Grass; width as usize]; height as usize];

    Map {
        height,
        width,
        coords,
    }
}

pub fn try_move_in_map(
    map: &Map,
    (center_x, center_y): Coords,
    step: Step,
    radius: u8,
    //player_coords: Vec<Coords>,
) -> Result<MoveCoords, ()> {
    let center_x = match step {
        Step::Up => center_x - 1,
        Step::Down => center_x + 1,
        _ => center_x,
    };
    let center_y = match step {
        Step::Left => center_y - 1,
        Step::Right => center_y + 1,
        _ => center_y,
    };

    if center_x < 0 || center_x >= map.height as i16 || center_y < 0 || center_y >= map.width as i16
    {
        return Err(());
    }

    //if player_coords.contains(&(center_x, center_y)) {
    //    return None;
    //}

    let radius_i16 = radius as i16;

    let top_left = (max(center_x - radius_i16, 0), max(center_y - radius_i16, 0));
    let bottom_right = (
        min(1 + center_x + radius_i16, map.height as i16),
        min(1 + center_y + radius_i16, map.width as i16),
    );

    assert!(top_left.0 <= bottom_right.0);
    assert!(top_left.1 <= bottom_right.1);

    let predicate: fn(i16, i16, i16) -> bool = match step {
        Step::Up => |x, y, r| x == ((r.pow(2) - y.pow(2)) as f64).sqrt() as i16,
        Step::Down => |x, y, r| x == -((r.pow(2) - y.pow(2)) as f64).sqrt() as i16,
        Step::Left => |x, y, r| y == ((r.pow(2) - x.pow(2)) as f64).sqrt() as i16,
        Step::Right => |x, y, r| y == -((r.pow(2) - x.pow(2)) as f64).sqrt() as i16,
    };

    let mut new_coords = vec![];
    for i in top_left.0..bottom_right.0 {
        for j in top_left.1..bottom_right.1 {
            // draw circle
            if predicate(center_x - i, center_y - j, radius_i16) {
                new_coords.push(MapCell::new(map.coords[i as usize][j as usize], (i, j)));
            }
        }
    }
    println!("new_coords.len: {}", new_coords.len());

    Ok(((center_x, center_y), new_coords))
}

pub fn is_inside_circle(
    (center_x, center_y): Coords,
    radius: u8,
    (other_x, other_y): Coords,
) -> bool {
    let diff_sqr = (center_x - other_x).pow(2) + (center_y - other_y).pow(2);
    let radius_sqr = (radius as i16).pow(2);

    diff_sqr <= radius_sqr
}
