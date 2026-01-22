use triforce::{Color, recomb, viz::*};

fn main() {
    // let data = [
    //     (mk_hex(0, 0), Color::WHITE),
    //     (mk_hex(0, 2), Color::WHITE),
    //     (mk_hex(0, 4), Color::from(basic::BLUE)),
    //     (mk_hex(0, 6), Color::from(basic::BLUE)),
    //     (mk_hex(0, 8), Color::BLACK),
    //     (mk_hex(1, 1), Color::WHITE),
    //     (mk_hex(1, 3), Color::WHITE),
    //     (mk_hex(1, 5), Color::WHITE),
    //     (mk_hex(1, 7), Color::WHITE),
    // ]
    // .into();

    let tiles = recomb(3)
        .into_iter()
        .filter(|(_, splits)| splits.len() > 1)
        .map(|(region, _)| {
            region
                .iter()
                .map(|(x, y)| (mk_hex(*x as i32, *y as i32), Color::new(1)))
                .collect()
        })
        .enumerate()
        .map(|(i, m)| (i.to_string(), m))
        .collect::<Vec<_>>();

    println!("there are {} recomb regions of interest", tiles.len());

    render(RenderData { tilings: tiles }, "out".to_string());
}
