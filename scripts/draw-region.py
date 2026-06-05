#!/usr/bin/env python3

from __future__ import annotations
from math import sqrt

import argparse
import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class Inputs:
    fixed_region: Any
    allowed: Any
    partial_tiling: Any


@dataclass(frozen=True)
class Cli:
    fixed_region: Path
    allowed: Path
    partial_tiling: Path
    neato_args: list[str]


def parse_args(argv: list[str] | None = None) -> Cli:
    parser = argparse.ArgumentParser(
        description="Draw given gadget.",
    )
    parser.add_argument("fixed_region", type=Path, help="JSON file containing the fixed region")
    parser.add_argument("allowed", type=Path, help="JSON file containing allowed tiles/data")
    parser.add_argument("partial_tiling", type=Path, help="JSON file containing the partial tiling")
    parser.add_argument("neato_args", nargs=argparse.REMAINDER, help=argparse.SUPPRESS)

    args = parser.parse_args(argv)
    neato_args = args.neato_args
    if neato_args[:1] == ["--"]:
        neato_args = neato_args[1:]

    return Cli(
        fixed_region=args.fixed_region,
        allowed=args.allowed,
        partial_tiling=args.partial_tiling,
        neato_args=neato_args,
    )


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)

SCALE = 50

def render(
        fixed_region: list[list[int]],
        allowed: list[list[int]],
        partial_tiling: list[list[list[int]]],
        prev_tiling = [[[0,-2],[0,-4]],[[1,-1],[1,-3]]],
        ) -> str:
    """Build a DOT graph for the requested region."""

    def mk_point(p: list[int]) -> tuple[int, int]:
        '''Restore the points for the sake of typing.'''
        [x, y] = p
        return (x, y)

    fixed_region: set[tuple[int, int]] = set(map(tuple, fixed_region))
    allowed: set[tuple[int, int]] = set(map(tuple, allowed))
    partial_tiling: list[set[tuple[int, int]]] = [set(map(tuple, tile)) for tile in partial_tiling]

    assert fixed_region.issubset(allowed)
    for tile in partial_tiling:
        assert tile.issubset(fixed_region)

    def name(node) -> str:
        n = f'n{node[0]}{node[1]}'
        return n.replace('-','_')

    def mk_node(node, color) -> str:
        x = node[0] * sqrt(3) * SCALE
        y = int(((node[1])) * SCALE)
        return f'{name(node)} [pos="{-x},{-y}!", fillcolor={color}, color={color}]'

    node_defs = []

    for node in allowed:
        color = 'red' if node in fixed_region else 'blue'
        node_defs.append(mk_node(node, color))

    edge_defs = []

    for tile in partial_tiling:
        [s, t] = tile
        edge_defs.append(f'{name(s)} -- {name(t)}')

    w_node = SCALE / 72 / 2
    w_edge = w_node * 72 / 2

    black_nodes = []
    black_edges = []

    for tile in prev_tiling:
        [s, t] = tile
        black_nodes.append(mk_node(s, 'black'))
        black_nodes.append(mk_node(t, 'black'))
        black_edges.append(f'{name(s)} -- {name(t)}')

    return f'''graph region {{
    dpi=72
    node [shape=circle, style=filled, width={w_node:.2f}, label=""]
    edge [penwidth={w_edge:.2f}, color=orange]
    {'    \n'.join(node_defs)}
    {'    \n'.join(edge_defs)}
    edge [penwidth={w_edge:.2f}, color=black]
    {'    \n'.join(black_nodes)}
    {'    \n'.join(black_edges)}
}}
'''


def run_neato(dot: str, neato_args: list[str]) -> None:
    cmd = ["neato", "-n2", *neato_args]
    subprocess.run(cmd, input=dot, text=True, check=True)


def main() -> None:
    cli = parse_args()
    dot = render(
    fixed_region=read_json(cli.fixed_region),
    allowed=read_json(cli.allowed),
    partial_tiling=read_json(cli.partial_tiling),
    prev_tiling=[],
    )
    # run_neato(dot, cli.neato_args)
    print(dot)


if __name__ == "__main__":
    main()
