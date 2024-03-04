set term png size 1200,600

set output "blockchain-stack-usage.png"
set xlabel "block height"
set ylabel "elements"
set title "block stack usage"
set key top right
plot "chain-resource-usage.dat" using 1:2 with dots title "value stack depth", \
"chain-resource-usage.dat" using 1:3 with dots title "environment stack depth", \
"chain-resource-usage.dat" using 1:4 with dots title "operations stack depth"

set output "blockchain-heap-usage.png"
set title "block heap usage"
set ylabel "MB"

plot "chain-resource-usage.dat" using 1:($5)*8/1000000 with dots title "Atom size", \
"chain-resource-usage.dat" using 1:($6)*8/1000000 with dots title "Pair size", \
"chain-resource-usage.dat" using 1:($7)/1000000 with dots title "Heap size"

set output "block-execution-time.png"
set title "block validation time"
set ylabel "microseconds"

plot "chain-resource-usage.dat" using 1:13 with dots title "block generator"

set output "block-loading-time.png"
plot "chain-resource-usage.dat" using 1:11 with dots title "block decompress + parse", \
"chain-resource-usage.dat" using 1:12 with dots title "block reference lookup", \
"chain-resource-usage.dat" using 1:14 with dots title "conditions`

set output "block-wall-clock-delta.png"
set title "transaction block timestamp delta"
set xlabel "s"
plot "chain-resource-usage.dat" using 1:16 with dots title "block timestamp delta", \

set output "block-cost-vs-time.png"
set title "block CLVM execution time versus CLVM cost"
set xlabel "CLVM cost (millions)"
set ylabel "CLVM execution time (s)"
set key off
plot "chain-resource-usage.dat" using ($9/1000000):($13/1000000) with dots title "blocks", \

set output "blockchain-stack-usage-cdf.png"
set xlabel "elements"
set ylabel "fraction of blocks"
set title "block stack usage"
set xrange [0:5000]
set key bottom right
plot "chain-resource-usage-cdf.dat" using 2:1 with lines title "value stack depth", \
"chain-resource-usage-cdf.dat" using 3:1 with lines title "environment stack depth", \
"chain-resource-usage-cdf.dat" using 4:1 with lines title "operations stack depth"

set output "blockchain-heap-usage-cdf.png"
set title "block heap usage"
set xlabel "MB"
set xrange [0:30]

plot "chain-resource-usage-cdf.dat" using ($5)*8/1000000:1 with lines title "Atom size", \
"chain-resource-usage-cdf.dat" using ($6)*8/1000000:1 with lines title "Pair size", \
"chain-resource-usage-cdf.dat" using ($7)/1000000:1 with lines title "Heap size"

set output "blockchain-object-usage-cdf.png"

set title "block object usage"
set xlabel "count (million)"
set xrange [0:*]

plot "chain-resource-usage-cdf.dat" using ($5)/1000000:1 with lines title "Allocated atoms", \
"chain-resource-usage-cdf.dat" using ($6)/1000000:1 with lines title "Allocated pairs"

set output "blockchain-object-usage-cdf2.png"
set xrange [0:3]
replot

set output "block-execution-time-cdf.png"
set title "block validation time"
set xlabel "microseconds"
set xrange [0:300000]

plot "chain-resource-usage-cdf.dat" using 13:1 with lines title "block generator", \

set output "block-loading-time-cdf.png"
set xrange [0:2000]

plot "chain-resource-usage-cdf.dat" using 11:1 with lines title "block decompress + parse", \
"chain-resource-usage-cdf.dat" using 12:1 with lines title "block reference lookup", \
"chain-resource-usage-cdf.dat" using 14:1 with lines title "conditions"

set output "block-wall-clock-delta-cdf.png"
set title "transaction block timestamp delta"
set xrange [0:200]
set xlabel "s"

plot "chain-resource-usage-cdf.dat" using 16:1 with lines title "block time delta (wall-clock)"

set output "block-wall-clock-delta-cdf-zoom.png"
set xrange [0:20]

replot
