# Hard fork 2 condition costs

As part of hard fork 2, the consensus rules for limits to conditions were changed.

Prior to hard fork 2, every CoinSpend could emit any number of conditions, but
no more than 1024 _announcement_ conditions (including messages, create and
assert announcements).

Once the hard fork has activated, the 1024 limit is removed. Instead all
conditions after the 100 first (per CoinSpend) have an additional cost of 500
applied. This applies to all conditions, whether they are known or unknown,
including the `SOFTFORK` condition as well as `CREATE_COIN` and `AGG_SIG_*`
conditions.

The cost per condition was established by benchmarking the `AGG_SIG_*`
conditions, knowing their cost to be 1800000, compute the cost per nanosecond
and then apply that on all other conditions, to estimate what a proportional
cost would be. iDifferent systems vary widely in the proportions of actual CPU
cost, but the benchmark gives some guidance. The cost of 500 was in the upper
end among conditions that are expensive.

The following are the benchmarks for a few different systems:

## Threadripper

```
condition: 49 nano-per-cond: 677972.916 cost-per-nanosecond: 1.770
condition: 50 nano-per-cond: 671905.156 cost-per-nanosecond: 1.786
condition: 43 nano-per-cond: 675518.102 cost-per-nanosecond: 1.776
condition: 44 nano-per-cond: 680348.959 cost-per-nanosecond: 1.764
condition: 45 nano-per-cond: 667876.463 cost-per-nanosecond: 1.797
condition: 47 nano-per-cond: 668400.500 cost-per-nanosecond: 1.795
condition: 48 nano-per-cond: 669700.376 cost-per-nanosecond: 1.792
condition: 46 nano-per-cond: 666194.337 cost-per-nanosecond: 1.801
condition:  1 nano-per-cond:  7.732 computed-cost: 13.80
condition: 70 nano-per-cond: 43.477 computed-cost: 77.61
condition: 71 nano-per-cond: 55.557 computed-cost: 99.18
condition: 72 nano-per-cond: 60.761 computed-cost: 108.47
condition: 73 nano-per-cond: 36.580 computed-cost: 65.30
condition: 75 nano-per-cond: 51.860 computed-cost: 92.58
condition: 74 nano-per-cond: 61.210 computed-cost: 109.27
condition: 80 nano-per-cond: 62.217 computed-cost: 111.07
condition: 81 nano-per-cond: 68.105 computed-cost: 121.58
condition: 82 nano-per-cond: 56.412 computed-cost: 100.70
condition: 83 nano-per-cond: 40.627 computed-cost: 72.53
condition: 84 nano-per-cond: 70.653 computed-cost: 126.13
condition: 85 nano-per-cond: 48.299 computed-cost: 86.22
condition: 86 nano-per-cond: 51.227 computed-cost: 91.45
condition: 87 nano-per-cond: 30.290 computed-cost: 54.07
condition: 90 nano-per-cond: 67.463 computed-cost: 120.43
condition: 66 nano-per-cond: 222.322 computed-cost: 396.88
condition: 61 nano-per-cond: 46.482 computed-cost: 82.98
condition: 63 nano-per-cond: 92.362 computed-cost: 164.88
```

## Linear cost

To validate that the cost increase is in fact linear to the number of
conditions, each condition was measured in spends ranging from 1 to 500
condition, expecting CPU time to increase linearly with the number of
conditions to parse.

This turned out to be the case.

![send message condition](./condition-66.png)
