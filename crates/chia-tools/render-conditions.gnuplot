system("mkdir -p histograms")

bin(x, w) = w * floor(x / w) + w / 2.0

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
    set terminal png size 800,600
    set output sprintf("histograms/%s.png", name)
    set title name font ",14"
    set xlabel "nanoseconds per condition"
    set ylabel "count"
    set style fill solid 0.5 noborder
    set grid y

    file = sprintf("data/%s.dat", name)
    stats file using 1 nooutput
    nbins = 50
    bw = (STATS_max - STATS_min) / nbins
    if (bw == 0) { bw = 1.0 }

    set arrow 1 from STATS_mean, graph 0 to STATS_mean, graph 1 nohead lc rgb "red" lw 2 dt 2 front
    set arrow 2 from STATS_median, graph 0 to STATS_median, graph 1 nohead lc rgb "blue" lw 2 front
    set label 1 sprintf("avg %.1f", STATS_mean) at STATS_mean, graph 0.95 tc rgb "red" right offset -1,0 front
    set label 2 sprintf("med %.1f", STATS_median) at STATS_median, graph 0.85 tc rgb "blue" right offset -1,0 front

    set boxwidth bw
    plot file using (bin($1, bw)):(1.0) smooth frequency with boxes lc rgb "#4477AA" notitle

    unset arrow 1; unset arrow 2
    unset label 1; unset label 2
}

do for [i=0:63] {
    set terminal png size 800,600
    set output sprintf("histograms/SendMessage_0x%02x.png", i)
    set title sprintf("SEND/RECEIVE\\_MESSAGE mode 0x%02x", i) font ",14"
    set xlabel "nanoseconds per condition"
    set ylabel "count"
    set style fill solid 0.5 noborder
    set grid y

    file = sprintf("data/SendMessage_0x%02x.dat", i)
    stats file using 1 nooutput
    nbins = 50
    bw = (STATS_max - STATS_min) / nbins
    if (bw == 0) { bw = 1.0 }

    set arrow 1 from STATS_mean, graph 0 to STATS_mean, graph 1 nohead lc rgb "red" lw 2 dt 2 front
    set arrow 2 from STATS_median, graph 0 to STATS_median, graph 1 nohead lc rgb "blue" lw 2 front
    set label 1 sprintf("avg %.1f", STATS_mean) at STATS_mean, graph 0.95 tc rgb "red" right offset -1,0 front
    set label 2 sprintf("med %.1f", STATS_median) at STATS_median, graph 0.85 tc rgb "blue" right offset -1,0 front

    set boxwidth bw
    plot file using (bin($1, bw)):(1.0) smooth frequency with boxes lc rgb "#4477AA" notitle

    unset arrow 1; unset arrow 2
    unset label 1; unset label 2
}
