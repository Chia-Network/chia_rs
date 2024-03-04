from typing import Dict, List

all_counters: Dict[str, List[int]] = {}

keys = ["val_stack:",
     "env_stack:",
     "op_stack:",
     "atoms:",
     "pairs:",
     "heap:",
     "block_cost:",
     "clvm_cost:",
     "cond_cost:",
     "parse_time:",
     "ref_lookup_time:",
     "execute_time:",
     "conditions_time:",
     "timestamp:",
     "time_delta:",
]

def to_int(value: str) -> int:
    if value[-1] == ',':
        return int(value[:-1])
    return int(value)

num_samples = 0

with open("chain-resource-usage.log", "r") as f:
    for l in f:
        cols = l.split()
        height = to_int(cols[0])
        all_counters.setdefault("height", []).append(height)
        for k in keys:
            i = cols.index(k)
            v = to_int(cols[i + 1])
            all_counters.setdefault(k, []).append(v)
        num_samples += 1

with open("chain-resource-usage.dat", "w+") as out:
    out.write("# ")
    for k in keys:
        out.write(f"{k} ")
    out.write(f"\n")

    for i in range(num_samples):
        out.write(f"{all_counters['height'][i]}")
        for k in keys:
            out.write(f" {all_counters[k][i]}")
        out.write(f"\n")

for k in keys:
    all_counters[k] = sorted(all_counters[k])

with open("chain-resource-usage-cdf.dat", "w+") as out:
    out.write("# ")
    for k in keys:
        out.write(f"{k} ")
    out.write(f"\n")

    for i in range(num_samples):
        out.write(f"{i/num_samples:0.3f}")
        for k in keys:
            out.write(f" {all_counters[k][i]}")
        out.write(f"\n")
