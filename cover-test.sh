#!/usr/bin/env zsh

time RAYON_NUM_THREADS=8 RUST_BACKTRACE=1 cargo run --release --bin search-covers -- triangle=6 triangle=6 triangle=7 2 line-2.json --exact-cover

