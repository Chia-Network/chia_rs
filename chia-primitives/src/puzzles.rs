use hex_literal::hex;

pub const P2_DELEGATED_OR_HIDDEN: [u8; 227] = hex!(
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

pub const P2_DELEGATED_OR_HIDDEN_HASH: [u8; 32] = hex!(
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

pub const CAT: [u8; 1672] = hex!(
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
