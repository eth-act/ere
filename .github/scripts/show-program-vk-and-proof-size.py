#!/usr/bin/env python3

import math
import os


def format_size(size):
    if size < 1024:
        return str(size)
    for unit in "KM":
        size /= 1024
        if size < 1024:
            return (
                f"{math.ceil(size)} {unit}iB"
                if size >= 10
                else f"{math.ceil(size * 10) / 10:.1f} {unit}iB"
            )
    return f"{math.ceil(size)} MiB"


def print_table(header, body):
    widths = [
        max(len(row[col]) for row in [header] + body) for col in range(len(header))
    ]
    for row in [header, ("-" * width for width in widths)] + body:
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
        os.path.getsize(f"crates/verifier/{zkvm}/tests/fixtures/{obj}.bin")
    )
    for zkvm in ZKVM
    for obj in ("program_vk", "proof")
}

print_table(
    ("zkVM", "ProgramVK", "Proof"),
    [(ZKVM[zkvm], sizes[zkvm, "program_vk"], sizes[zkvm, "proof"]) for zkvm in ZKVM],
)
