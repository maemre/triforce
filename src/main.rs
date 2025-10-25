use triforce::*;

fn main() {
    let k = 3;
    // list regions of size k
    for r in regions(k) {
        let mut v = r.iter().collect::<Vec<_>>();
        v.sort();
        println!("{v:?}");
    }

    for g in Graph::enumerate(3, 3) {
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
            // println!("{k}");
            let gs = Graph::enumerate(n, k);
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
