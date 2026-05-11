#!/usr/bin/env python3

import math
import os


def h(n):
    if n < 1024:
        return str(n)
    for u in "KMGT":
        n /= 1024
        if n < 1024:
            return f"{math.ceil(n)}{u}" if n >= 10 else f"{math.ceil(n * 10) / 10:.1f}{u}"
    return f"{math.ceil(n)}T"


D = {
    "airbender": "Airbender",
    "openvm": "OpenVM",
    "risc0": "RISC Zero",
    "sp1": "SP1",
    "zisk": "ZisK",
}

s = {
    (z, k): h(os.path.getsize(f"crates/verifier/{z}/tests/fixtures/{k}.bin"))
    for z in D
    for k in ("program_vk", "proof")
}

rows = [("zkvm", "program vk", "proof")] + [
    (D[z], s[z, "program_vk"], s[z, "proof"]) for z in D
]
w = [max(len(r[i]) for r in rows) for i in range(3)]
row = lambda r: "| " + " | ".join(c.ljust(w[i]) for i, c in enumerate(r)) + " |"

print(row(rows[0]))
print("| " + " | ".join("-" * x for x in w) + " |")
for r in rows[1:]:
    print(row(r))
