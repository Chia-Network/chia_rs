use hex_literal::hex;

pub const STANDARD_PUZZLE: [u8; 227] = hex!(
    "
    ff02ffff01ff02ffff03ff0bffff01ff02ffff03ffff09ff05ffff1dff0bffff
    1effff0bff0bffff02ff06ffff04ff02ffff04ff17ff8080808080808080ffff
    01ff02ff17ff2f80ffff01ff088080ff0180ffff01ff04ffff04ff04ffff04ff
    05ffff04ffff02ff06ffff04ff02ffff04ff17ff80808080ff80808080ffff02
    ff17ff2f808080ff0180ffff04ffff01ff32ff02ffff03ffff07ff0580ffff01
    ff0bffff0102ffff02ff06ffff04ff02ffff04ff09ff80808080ffff02ff06ff
    ff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff
    018080
    "
);

pub const STANDARD_PUZZLE_HASH: [u8; 32] = hex!(
    "
    e9aaa49f45bad5c889b86ee3341550c155cfdd10c3a6757de618d20612fffd52
    "
);

pub const DEFAULT_HIDDEN_PUZZLE: [u8; 3] = hex!("ff0980");

pub const DEFAULT_HIDDEN_PUZZLE_HASH: [u8; 32] = hex!(
    "
    711d6c4e32c92e53179b199484cf8c897542bc57f2b22582799f9d657eec4699
    "
);

pub const CAT_PUZZLE: [u8; 1672] = hex!(
    "
    ff02ffff01ff02ff5effff04ff02ffff04ffff04ff05ffff04ffff0bff34ff05
    80ffff04ff0bff80808080ffff04ffff02ff17ff2f80ffff04ff5fffff04ffff
    02ff2effff04ff02ffff04ff17ff80808080ffff04ffff02ff2affff04ff02ff
    ff04ff82027fffff04ff82057fffff04ff820b7fff808080808080ffff04ff81
    bfffff04ff82017fffff04ff8202ffffff04ff8205ffffff04ff820bffff8080
    8080808080808080808080ffff04ffff01ffffffff3d46ff02ff333cffff0401
    ff01ff81cb02ffffff20ff02ffff03ff05ffff01ff02ff32ffff04ff02ffff04
    ff0dffff04ffff0bff7cffff0bff34ff2480ffff0bff7cffff0bff7cffff0bff
    34ff2c80ff0980ffff0bff7cff0bffff0bff34ff8080808080ff8080808080ff
    ff010b80ff0180ffff02ffff03ffff22ffff09ffff0dff0580ff2280ffff09ff
    ff0dff0b80ff2280ffff15ff17ffff0181ff8080ffff01ff0bff05ff0bff1780
    ffff01ff088080ff0180ffff02ffff03ff0bffff01ff02ffff03ffff09ffff02
    ff2effff04ff02ffff04ff13ff80808080ff820b9f80ffff01ff02ff56ffff04
    ff02ffff04ffff02ff13ffff04ff5fffff04ff17ffff04ff2fffff04ff81bfff
    ff04ff82017fffff04ff1bff8080808080808080ffff04ff82017fff80808080
    80ffff01ff088080ff0180ffff01ff02ffff03ff17ffff01ff02ffff03ffff20
    ff81bf80ffff0182017fffff01ff088080ff0180ffff01ff088080ff018080ff
    0180ff04ffff04ff05ff2780ffff04ffff10ff0bff5780ff778080ffffff02ff
    ff03ff05ffff01ff02ffff03ffff09ffff02ffff03ffff09ff11ff5880ffff01
    59ff8080ff0180ffff01818f80ffff01ff02ff26ffff04ff02ffff04ff0dffff
    04ff0bffff04ffff04ff81b9ff82017980ff808080808080ffff01ff02ff7aff
    ff04ff02ffff04ffff02ffff03ffff09ff11ff5880ffff01ff04ff58ffff04ff
    ff02ff76ffff04ff02ffff04ff13ffff04ff29ffff04ffff0bff34ff5b80ffff
    04ff2bff80808080808080ff398080ffff01ff02ffff03ffff09ff11ff7880ff
    ff01ff02ffff03ffff20ffff02ffff03ffff09ffff0121ffff0dff298080ffff
    01ff02ffff03ffff09ffff0cff29ff80ff3480ff5c80ffff01ff0101ff8080ff
    0180ff8080ff018080ffff0109ffff01ff088080ff0180ffff010980ff018080
    ff0180ffff04ffff02ffff03ffff09ff11ff5880ffff0159ff8080ff0180ffff
    04ffff02ff26ffff04ff02ffff04ff0dffff04ff0bffff04ff17ff8080808080
    80ff80808080808080ff0180ffff01ff04ff80ffff04ff80ff17808080ff0180
    ffff02ffff03ff05ffff01ff04ff09ffff02ff56ffff04ff02ffff04ff0dffff
    04ff0bff808080808080ffff010b80ff0180ff0bff7cffff0bff34ff2880ffff
    0bff7cffff0bff7cffff0bff34ff2c80ff0580ffff0bff7cffff02ff32ffff04
    ff02ffff04ff07ffff04ffff0bff34ff3480ff8080808080ffff0bff34ff8080
    808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff2effff04
    ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff0dff80808080
    80ffff01ff0bffff0101ff058080ff0180ffff04ffff04ff30ffff04ff5fff80
    8080ffff02ff7effff04ff02ffff04ffff04ffff04ff2fff0580ffff04ff5fff
    82017f8080ffff04ffff02ff26ffff04ff02ffff04ff0bffff04ff05ffff01ff
    808080808080ffff04ff17ffff04ff81bfffff04ff82017fffff04ffff02ff2a
    ffff04ff02ffff04ff8204ffffff04ffff02ff76ffff04ff02ffff04ff09ffff
    04ff820affffff04ffff0bff34ff2d80ffff04ff15ff80808080808080ffff04
    ff8216ffff808080808080ffff04ff8205ffffff04ff820bffff808080808080
    808080808080ff02ff5affff04ff02ffff04ff5fffff04ff3bffff04ffff02ff
    ff03ff17ffff01ff09ff2dffff02ff2affff04ff02ffff04ff27ffff04ffff02
    ff76ffff04ff02ffff04ff29ffff04ff57ffff04ffff0bff34ff81b980ffff04
    ff59ff80808080808080ffff04ff81b7ff80808080808080ff8080ff0180ffff
    04ff17ffff04ff05ffff04ff8202ffffff04ffff04ffff04ff78ffff04ffff0e
    ff5cffff02ff2effff04ff02ffff04ffff04ff2fffff04ff82017fff808080ff
    8080808080ff808080ffff04ffff04ff20ffff04ffff0bff81bfff5cffff02ff
    2effff04ff02ffff04ffff04ff15ffff04ffff10ff82017fffff11ff8202dfff
    2b80ff8202ff80ff808080ff8080808080ff808080ff138080ff808080808080
    80808080ff018080
    "
);

pub const CAT_PUZZLE_HASH: [u8; 32] = hex!(
    "
    37bef360ee858133b69d595a906dc45d01af50379dad515eb9518abb7c1d2a7a
    "
);

pub const LAUNCHER_PUZZLE: [u8; 175] = hex!(
    "
    ff02ffff01ff04ffff04ff04ffff04ff05ffff04ff0bff80808080ffff04ffff
    04ff0affff04ffff02ff0effff04ff02ffff04ffff04ff05ffff04ff0bffff04
    ff17ff80808080ff80808080ff808080ff808080ffff04ffff01ff33ff3cff02
    ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff0effff04ff02ffff04ff
    09ff80808080ffff02ff0effff04ff02ffff04ff0dff8080808080ffff01ff0b
    ffff0101ff058080ff0180ff018080
    "
);

pub const LAUNCHER_PUZZLE_HASH: [u8; 32] = hex!(
    "
    eff07522495060c066f66f32acc2a77e3a3e737aca8baea4d1a64ea4cdc13da9
    "
);

pub const SINGLETON_PUZZLE: [u8; 967] = hex!(
    "
    ff02ffff01ff02ffff03ffff18ff2fff3480ffff01ff04ffff04ff20ffff04ff
    2fff808080ffff04ffff02ff3effff04ff02ffff04ff05ffff04ffff02ff2aff
    ff04ff02ffff04ff27ffff04ffff02ffff03ff77ffff01ff02ff36ffff04ff02
    ffff04ff09ffff04ff57ffff04ffff02ff2effff04ff02ffff04ff05ff808080
    80ff808080808080ffff011d80ff0180ffff04ffff02ffff03ff77ffff0181b7
    ffff015780ff0180ff808080808080ffff04ff77ff808080808080ffff02ff3a
    ffff04ff02ffff04ff05ffff04ffff02ff0bff5f80ffff01ff80808080808080
    80ffff01ff088080ff0180ffff04ffff01ffffffff4947ff0233ffff0401ff01
    02ffffff20ff02ffff03ff05ffff01ff02ff32ffff04ff02ffff04ff0dffff04
    ffff0bff3cffff0bff34ff2480ffff0bff3cffff0bff3cffff0bff34ff2c80ff
    0980ffff0bff3cff0bffff0bff34ff8080808080ff8080808080ffff010b80ff
    0180ffff02ffff03ffff22ffff09ffff0dff0580ff2280ffff09ffff0dff0b80
    ff2280ffff15ff17ffff0181ff8080ffff01ff0bff05ff0bff1780ffff01ff08
    8080ff0180ff02ffff03ff0bffff01ff02ffff03ffff02ff26ffff04ff02ffff
    04ff13ff80808080ffff01ff02ffff03ffff20ff1780ffff01ff02ffff03ffff
    09ff81b3ffff01818f80ffff01ff02ff3affff04ff02ffff04ff05ffff04ff1b
    ffff04ff34ff808080808080ffff01ff04ffff04ff23ffff04ffff02ff36ffff
    04ff02ffff04ff09ffff04ff53ffff04ffff02ff2effff04ff02ffff04ff05ff
    80808080ff808080808080ff738080ffff02ff3affff04ff02ffff04ff05ffff
    04ff1bffff04ff34ff8080808080808080ff0180ffff01ff088080ff0180ffff
    01ff04ff13ffff02ff3affff04ff02ffff04ff05ffff04ff1bffff04ff17ff80
    80808080808080ff0180ffff01ff02ffff03ff17ff80ffff01ff088080ff0180
    80ff0180ffffff02ffff03ffff09ff09ff3880ffff01ff02ffff03ffff18ff2d
    ffff010180ffff01ff0101ff8080ff0180ff8080ff0180ff0bff3cffff0bff34
    ff2880ffff0bff3cffff0bff3cffff0bff34ff2c80ff0580ffff0bff3cffff02
    ff32ffff04ff02ffff04ff07ffff04ffff0bff34ff3480ff8080808080ffff0b
    ff34ff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02
    ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff0d
    ff8080808080ffff01ff0bffff0101ff058080ff0180ff02ffff03ffff21ff17
    ffff09ff0bff158080ffff01ff04ff30ffff04ff0bff808080ffff01ff088080
    ff0180ff018080
    "
);

pub const SINGLETON_PUZZLE_HASH: [u8; 32] = hex!(
    "
    7faa3253bfddd1e0decb0906b2dc6247bbc4cf608f58345d173adb63e8b47c9f
    "
);

pub const DID_INNER_PUZZLE: [u8; 1012] = hex!(
    "
    ff02ffff01ff02ffff03ff81bfffff01ff02ff05ff82017f80ffff01ff02ffff
    03ffff22ffff09ffff02ff7effff04ff02ffff04ff8217ffff80808080ff0b80
    ffff15ff17ff808080ffff01ff04ffff04ff28ffff04ff82017fff808080ffff
    04ffff04ff34ffff04ff8202ffffff04ff82017fffff04ffff04ff8202ffff80
    80ff8080808080ffff04ffff04ff38ffff04ff822fffff808080ffff02ff26ff
    ff04ff02ffff04ff2fffff04ff17ffff04ff8217ffffff04ff822fffffff04ff
    8202ffffff04ff8205ffffff04ff820bffffff01ff8080808080808080808080
    808080ffff01ff088080ff018080ff0180ffff04ffff01ffffffff313dff4946
    ffff0233ff3c04ffffff0101ff02ff02ffff03ff05ffff01ff02ff3affff04ff
    02ffff04ff0dffff04ffff0bff2affff0bff22ff3c80ffff0bff2affff0bff2a
    ffff0bff22ff3280ff0980ffff0bff2aff0bffff0bff22ff8080808080ff8080
    808080ffff010b80ff0180ffffff02ffff03ff17ffff01ff02ffff03ff82013f
    ffff01ff04ffff04ff30ffff04ffff0bffff0bffff02ff36ffff04ff02ffff04
    ff05ffff04ff27ffff04ff82023fffff04ff82053fffff04ff820b3fff808080
    8080808080ffff02ff7effff04ff02ffff04ffff02ff2effff04ff02ffff04ff
    2fffff04ff5fffff04ff82017fff808080808080ff8080808080ff2f80ff8080
    80ffff02ff26ffff04ff02ffff04ff05ffff04ff0bffff04ff37ffff04ff2fff
    ff04ff5fffff04ff8201bfffff04ff82017fffff04ffff10ff8202ffffff0101
    80ff808080808080808080808080ffff01ff02ff26ffff04ff02ffff04ff05ff
    ff04ff37ffff04ff2fffff04ff5fffff04ff8201bfffff04ff82017fffff04ff
    8202ffff8080808080808080808080ff0180ffff01ff02ffff03ffff15ff8202
    ffffff11ff0bffff01018080ffff01ff04ffff04ff20ffff04ff82017fffff04
    ff5fff80808080ff8080ffff01ff088080ff018080ff0180ff0bff17ffff02ff
    5effff04ff02ffff04ff09ffff04ff2fffff04ffff02ff7effff04ff02ffff04
    ffff04ff09ffff04ff0bff1d8080ff80808080ff808080808080ff5f80ffff04
    ffff0101ffff04ffff04ff2cffff04ff05ff808080ffff04ffff04ff20ffff04
    ff17ffff04ff0bff80808080ff80808080ffff0bff2affff0bff22ff2480ffff
    0bff2affff0bff2affff0bff22ff3280ff0580ffff0bff2affff02ff3affff04
    ff02ffff04ff07ffff04ffff0bff22ff2280ff8080808080ffff0bff22ff8080
    808080ff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff7effff04ff
    02ffff04ff09ff80808080ffff02ff7effff04ff02ffff04ff0dff8080808080
    ffff01ff0bffff0101ff058080ff0180ff018080
    "
);

pub const DID_INNER_PUZZLE_HASH: [u8; 32] = hex!(
    "
    33143d2bef64f14036742673afd158126b94284b4530a28c354fac202b0c910e
    "
);

pub const NFT_STATE_LAYER_PUZZLE: [u8; 827] = hex!(
    "
    ff02ffff01ff02ff3effff04ff02ffff04ff05ffff04ffff02ff2fff5f80ffff
    04ff80ffff04ffff04ffff04ff0bffff04ff17ff808080ffff01ff808080ffff
    01ff8080808080808080ffff04ffff01ffffff0233ff04ff0101ffff02ff02ff
    ff03ff05ffff01ff02ff1affff04ff02ffff04ff0dffff04ffff0bff12ffff0b
    ff2cff1480ffff0bff12ffff0bff12ffff0bff2cff3c80ff0980ffff0bff12ff
    0bffff0bff2cff8080808080ff8080808080ffff010b80ff0180ffff0bff12ff
    ff0bff2cff1080ffff0bff12ffff0bff12ffff0bff2cff3c80ff0580ffff0bff
    12ffff02ff1affff04ff02ffff04ff07ffff04ffff0bff2cff2c80ff80808080
    80ffff0bff2cff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff01
    02ffff02ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ff
    ff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff02ffff03ff
    0bffff01ff02ffff03ffff09ff23ff1880ffff01ff02ffff03ffff18ff81b3ff
    2c80ffff01ff02ffff03ffff20ff1780ffff01ff02ff3effff04ff02ffff04ff
    05ffff04ff1bffff04ff33ffff04ff2fffff04ff5fff8080808080808080ffff
    01ff088080ff0180ffff01ff04ff13ffff02ff3effff04ff02ffff04ff05ffff
    04ff1bffff04ff17ffff04ff2fffff04ff5fff80808080808080808080ff0180
    ffff01ff02ffff03ffff09ff23ffff0181e880ffff01ff02ff3effff04ff02ff
    ff04ff05ffff04ff1bffff04ff17ffff04ffff02ffff03ffff22ffff09ffff02
    ff2effff04ff02ffff04ff53ff80808080ff82014f80ffff20ff5f8080ffff01
    ff02ff53ffff04ff818fffff04ff82014fffff04ff81b3ff8080808080ffff01
    ff088080ff0180ffff04ff2cff8080808080808080ffff01ff04ff13ffff02ff
    3effff04ff02ffff04ff05ffff04ff1bffff04ff17ffff04ff2fffff04ff5fff
    80808080808080808080ff018080ff0180ffff01ff04ffff04ff18ffff04ffff
    02ff16ffff04ff02ffff04ff05ffff04ff27ffff04ffff0bff2cff82014f80ff
    ff04ffff02ff2effff04ff02ffff04ff818fff80808080ffff04ffff0bff2cff
    0580ff8080808080808080ff378080ff81af8080ff0180ff018080
    "
);

pub const NFT_STATE_LAYER_PUZZLE_HASH: [u8; 32] = hex!(
    "
    a04d9f57764f54a43e4030befb4d80026e870519aaa66334aef8304f5d0393c2
    "
);

pub const NFT_OWNERSHIP_LAYER_PUZZLE: [u8; 1226] = hex!(
    "
    ff02ffff01ff02ff26ffff04ff02ffff04ff05ffff04ff17ffff04ff0bffff04
    ffff02ff2fff5f80ff80808080808080ffff04ffff01ffffff82ad4cff0233ff
    ff3e04ff81f601ffffff0102ffff02ffff03ff05ffff01ff02ff2affff04ff02
    ffff04ff0dffff04ffff0bff32ffff0bff3cff3480ffff0bff32ffff0bff32ff
    ff0bff3cff2280ff0980ffff0bff32ff0bffff0bff3cff8080808080ff808080
    8080ffff010b80ff0180ff04ffff04ff38ffff04ffff02ff36ffff04ff02ffff
    04ff05ffff04ff27ffff04ffff02ff2effff04ff02ffff04ffff02ffff03ff81
    afffff0181afffff010b80ff0180ff80808080ffff04ffff0bff3cff4f80ffff
    04ffff0bff3cff0580ff8080808080808080ff378080ff82016f80ffffff02ff
    3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff2fff
    ff01ff80ff808080808080808080ff0bff32ffff0bff3cff2880ffff0bff32ff
    ff0bff32ffff0bff3cff2280ff0580ffff0bff32ffff02ff2affff04ff02ffff
    04ff07ffff04ffff0bff3cff3c80ff8080808080ffff0bff3cff8080808080ff
    ff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff2effff04ff02ffff
    04ff09ff80808080ffff02ff2effff04ff02ffff04ff0dff8080808080ffff01
    ff0bffff0101ff058080ff0180ff02ffff03ff5fffff01ff02ffff03ffff09ff
    82011fff3880ffff01ff02ffff03ffff09ffff18ff82059f80ff3c80ffff01ff
    02ffff03ffff20ff81bf80ffff01ff02ff3effff04ff02ffff04ff05ffff04ff
    0bffff04ff17ffff04ff2fffff04ff81dfffff04ff82019fffff04ff82017fff
    80808080808080808080ffff01ff088080ff0180ffff01ff04ff819fffff02ff
    3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81df
    ffff04ff81bfffff04ff82017fff808080808080808080808080ff0180ffff01
    ff02ffff03ffff09ff82011fff2c80ffff01ff02ffff03ffff20ff82017f80ff
    ff01ff04ffff04ff24ffff04ffff0eff10ffff02ff2effff04ff02ffff04ff82
    019fff8080808080ff808080ffff02ff3effff04ff02ffff04ff05ffff04ff0b
    ffff04ff17ffff04ff2fffff04ff81dfffff04ff81bfffff04ffff02ff0bffff
    04ff17ffff04ff2fffff04ff82019fff8080808080ff80808080808080808080
    80ffff01ff088080ff0180ffff01ff02ffff03ffff09ff82011fff2480ffff01
    ff02ffff03ffff20ffff02ffff03ffff09ffff0122ffff0dff82029f8080ffff
    01ff02ffff03ffff09ffff0cff82029fff80ffff010280ff1080ffff01ff0101
    ff8080ff0180ff8080ff018080ffff01ff04ff819fffff02ff3effff04ff02ff
    ff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81dfffff04ff81bfff
    ff04ff82017fff8080808080808080808080ffff01ff088080ff0180ffff01ff
    04ff819fffff02ff3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04
    ff2fffff04ff81dfffff04ff81bfffff04ff82017fff80808080808080808080
    8080ff018080ff018080ff0180ffff01ff02ff3affff04ff02ffff04ff05ffff
    04ff0bffff04ff81bfffff04ffff02ffff03ff82017fffff0182017fffff01ff
    02ff0bffff04ff17ffff04ff2fffff01ff808080808080ff0180ff8080808080
    808080ff0180ff018080
    "
);

pub const NFT_OWNERSHIP_LAYER_PUZZLE_HASH: [u8; 32] = hex!(
    "
    c5abea79afaa001b5427dfa0c8cf42ca6f38f5841b78f9b3c252733eb2de2726
    "
);

pub const NFT_ROYALTY_TRANSFER_PUZZLE: [u8; 687] = hex!(
    "
    ff02ffff01ff02ffff03ff81bfffff01ff04ff82013fffff04ff80ffff04ffff
    02ffff03ffff22ff82013fffff20ffff09ff82013fff2f808080ffff01ff04ff
    ff04ff10ffff04ffff0bffff02ff2effff04ff02ffff04ff09ffff04ff8205bf
    ffff04ffff02ff3effff04ff02ffff04ffff04ff09ffff04ff82013fff1d8080
    ff80808080ff808080808080ff1580ff808080ffff02ff16ffff04ff02ffff04
    ff0bffff04ff17ffff04ff8202bfffff04ff15ff8080808080808080ffff01ff
    02ff16ffff04ff02ffff04ff0bffff04ff17ffff04ff8202bfffff04ff15ff80
    80808080808080ff0180ff80808080ffff01ff04ff2fffff01ff80ff80808080
    ff0180ffff04ffff01ffffff3f02ff04ff0101ffff822710ff02ff02ffff03ff
    05ffff01ff02ff3affff04ff02ffff04ff0dffff04ffff0bff2affff0bff2cff
    1480ffff0bff2affff0bff2affff0bff2cff3c80ff0980ffff0bff2aff0bffff
    0bff2cff8080808080ff8080808080ffff010b80ff0180ffff02ffff03ff17ff
    ff01ff04ffff04ff10ffff04ffff0bff81a7ffff02ff3effff04ff02ffff04ff
    ff04ff2fffff04ffff04ff05ffff04ffff05ffff14ffff12ff47ff0b80ff1280
    80ffff04ffff04ff05ff8080ff80808080ff808080ff8080808080ff808080ff
    ff02ff16ffff04ff02ffff04ff05ffff04ff0bffff04ff37ffff04ff2fff8080
    808080808080ff8080ff0180ffff0bff2affff0bff2cff1880ffff0bff2affff
    0bff2affff0bff2cff3c80ff0580ffff0bff2affff02ff3affff04ff02ffff04
    ff07ffff04ffff0bff2cff2c80ff8080808080ffff0bff2cff8080808080ff02
    ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff3effff04ff02ffff04ff
    09ff80808080ffff02ff3effff04ff02ffff04ff0dff8080808080ffff01ff0b
    ffff0101ff058080ff0180ff018080
    "
);

pub const NFT_ROYALTY_TRANSFER_PUZZLE_HASH: [u8; 32] = hex!(
    "
    025dee0fb1e9fa110302a7e9bfb6e381ca09618e2778b0184fa5c6b275cfce1f
    "
);

pub const NFT_METADATA_UPDATER_PUZZLE: [u8; 241] = hex!(
    "
    ff02ffff01ff04ffff04ffff02ffff03ffff22ff27ff3780ffff01ff02ffff03
    ffff21ffff09ff27ffff01826d7580ffff09ff27ffff01826c7580ffff09ff27
    ffff01758080ffff01ff02ff02ffff04ff02ffff04ff05ffff04ff27ffff04ff
    37ff808080808080ffff010580ff0180ffff010580ff0180ffff04ff0bff8080
    80ffff01ff808080ffff04ffff01ff02ffff03ff05ffff01ff02ffff03ffff09
    ff11ff0b80ffff01ff04ffff04ff0bffff04ff17ff198080ff0d80ffff01ff04
    ff09ffff02ff02ffff04ff02ffff04ff0dffff04ff0bffff04ff17ff80808080
    80808080ff0180ff8080ff0180ff018080
    "
);

pub const NFT_METADATA_UPDATER_PUZZLE_HASH: [u8; 32] = hex!(
    "
    fe8a4b4e27a2e29a4d3fc7ce9d527adbcaccbab6ada3903ccf3ba9a769d2d78b
    "
);

pub const NFT_INTERMEDIATE_LAUNCHER_PUZZLE: [u8; 65] = hex!(
    "
    ff02ffff01ff04ffff04ff04ffff04ff05ffff01ff01808080ffff04ffff04ff
    06ffff04ffff0bff0bff1780ff808080ff808080ffff04ffff01ff333cff0180
    80
    "
);

pub const NFT_INTERMEDIATE_LAUNCHER_PUZZLE_HASH: [u8; 32] = hex!(
    "
    7a32d2d9571d3436791c0ad3d7fcfdb9c43ace2b0f0ff13f98d29f0cc093f445
    "
);
