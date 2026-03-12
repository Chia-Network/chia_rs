system("mkdir -p histograms")

conditions = "AggSigUnsafe AggSigMe AggSigParent AggSigPuzzle AggSigAmount \
AggSigParentAmount AggSigParentPuzzle AggSigPuzzleAmount Remark \
AssertMyCoinId AssertMyParentId AssertMyPuzzlehash AssertMyAmount \
AssertMyBirthHeight AssertMyBirthSeconds AssertSecondsRelative \
AssertSecondsAbsolute AssertHeightRelative AssertHeightAbsolute \
AssertBeforeSecondsRelative AssertBeforeSecondsAbsolute \
AssertBeforeHeightRelative AssertBeforeHeightAbsolute Softfork \
AssertConcurrentSpend AssertConcurrentPuzzle AssertEphemeral \
CreateCoinAnnouncement AssertCoinAnnouncement \
CreatePuzzleAnnouncement AssertPuzzleAnnouncement"

do for [name in conditions] {
    file = sprintf("data/%s.dat", name)
    stats file using 1 nooutput

    set terminal png size 800,600
    set output sprintf("histograms/%s.png", name)
    set title name font ",14"
    set xlabel "nanoseconds per condition"
    set ylabel "cumulative probability"
    set yrange [0:1]
    set grid

    set arrow 1 from STATS_mean, graph 0 to STATS_mean, graph 1 nohead lc rgb "red" lw 2 dt 2 front
    set arrow 2 from STATS_median, graph 0 to STATS_median, graph 1 nohead lc rgb "blue" lw 2 front
    set label 1 sprintf("avg %.1f", STATS_mean) at STATS_mean, graph 0.95 tc rgb "red" right offset -1,0 front
    set label 2 sprintf("med %.1f", STATS_median) at STATS_median, graph 0.85 tc rgb "blue" right offset -1,0 front

    plot file using 1:(1.0) smooth cnormal with lines lw 2 lc rgb "#4477AA" notitle

    unset arrow 1; unset arrow 2
    unset label 1; unset label 2
    unset yrange
}

do for [i=0:63] {
    file = sprintf("data/SendMessage_0x%02x.dat", i)
    stats file using 1 nooutput

    set terminal png size 800,600
    set output sprintf("histograms/SendMessage_0x%02x.png", i)
    set title sprintf("SEND/RECEIVE\\_MESSAGE mode 0x%02x", i) font ",14"
    set xlabel "nanoseconds per condition"
    set ylabel "cumulative probability"
    set yrange [0:1]
    set grid

    set arrow 1 from STATS_mean, graph 0 to STATS_mean, graph 1 nohead lc rgb "red" lw 2 dt 2 front
    set arrow 2 from STATS_median, graph 0 to STATS_median, graph 1 nohead lc rgb "blue" lw 2 front
    set label 1 sprintf("avg %.1f", STATS_mean) at STATS_mean, graph 0.95 tc rgb "red" right offset -1,0 front
    set label 2 sprintf("med %.1f", STATS_median) at STATS_median, graph 0.85 tc rgb "blue" right offset -1,0 front

    plot file using 1:(1.0) smooth cnormal with lines lw 2 lc rgb "#4477AA" notitle

    unset arrow 1; unset arrow 2
    unset label 1; unset label 2
    unset yrange
}
