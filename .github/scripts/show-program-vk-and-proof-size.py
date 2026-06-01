#!/usr/bin/env python3

import math
import os
from pathlib import Path

script_dir = Path(__file__).parent.resolve()


def format_size(size):
    if size < 1024:
        return f"{size} B"
    return f"{math.ceil(size / 1024)} KiB"


def print_table(header, sep, body):
    widths = [
        max(len(row[col]) for row in [header] + body) for col in range(len(header))
    ]
    for row in [header, sep] + body:
        print(
            "| "
            + " | ".join(cell.ljust(widths[col]) for col, cell in enumerate(row))
            + " |"
        )


ZKVM = {
    "airbender": "Airbender",
    "openvm": "OpenVM",
    "risc0": "RISC Zero",
    "sp1": "SP1",
    "zisk": "ZisK",
}

sizes = {
    (zkvm, obj): format_size(
        os.path.getsize(
            f"{script_dir}/../../crates/verifier/{zkvm}/tests/fixtures/{obj}.bin"
        )
    )
    for zkvm in ZKVM
    for obj in ("program_vk", "proof")
}

print_table(
    ("zkVM", "ProgramVK", "Proof"),
    ("-", "-:", "-:"),
    [(ZKVM[zkvm], sizes[zkvm, "program_vk"], sizes[zkvm, "proof"]) for zkvm in ZKVM],
)
