use hex_literal::hex;

pub const STANDARD_TRANSACTION: [u8; 227] = hex!(
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
