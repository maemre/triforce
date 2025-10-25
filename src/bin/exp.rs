use triforce::*;

fn main() {
    for r in recomb(2).keys() {
        let mut v = r.iter().collect::<Vec<_>>();
        v.sort();
        println!("{v:?}");
	println!("{}", serde_json::to_string(&MaybeRegion::from_region(r.clone())).unwrap());
    }

    println!("triangle(3): {}", serde_json::to_string(&MaybeRegion::from_region(Graph::triangle(3).into_region())).unwrap());


    for g in Tiling::enumerate(&Graph::triangle(3), 3) {
        println!("{g}\n");
    }

    println!("regions:");
    for k in 2..7 {
        println!("{k}: {:>7}", regions(k).len());
    }

    println!();

    println!("tilings:");
    println!("(n, k):  partial complete   recomb");
    println!("----------------------------------");
    for n in 3..9 {
        for k in 2..6 {
            // for k in 2..(n * (n + 1) / 2) {
            if (n * (n + 1) / 2) % k != 0 {
                continue;
            }
	    if n == 8 && k > 3 {
		break;
	    }
            // println!("{k}");
	    let triangle = Graph::triangle(n);
            let gs = Tiling::enumerate(&triangle, k);
            let complete = gs
                .iter()
                .filter(|g| g.is_complete())
                .collect::<Vec<_>>();
            let first = (*complete.iter().min().unwrap()).clone();
            let gs_len = gs.len();
            let complete_len = complete.len();
            drop(complete);
            drop(gs);
            // println!("{}", first);
            let reachable = first.reachable(k);
            println!(
                "({n}, {k}): {:>8} {:>8} {:>8}",
                gs_len,
                complete_len,
                reachable.len()
            );
            // println!("enumerated graphs:");
            // for g in complete {
            //     println!("{g}");
            // }
            // println!("reachable graphs:");
            // for g in reachable {
            //     println!("{g}");
            // }
        }
    }
}
