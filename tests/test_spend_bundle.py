from chia_rs import SpendBundle
from chia.types.spend_bundle import SpendBundle as PySpendBundle
import pytest
from typing import Type

expected_add1 = """\
[Coin { parent_coin_info: a48e6325fee4948d0703d1d722416331890e068d095a05049dd516ee7eab7f4b, \
puzzle_hash: cbc0619761e5f7687d78094fb94e484042d488839a4b4ee37b61eaccc17a1943, \
amount: 164000000000 }, \
Coin { parent_coin_info: a48e6325fee4948d0703d1d722416331890e068d095a05049dd516ee7eab7f4b, \
puzzle_hash: 5b22c1048a2e41e70bf633e8b189231949f35e26e160fa6ef49114232003435c, \
amount: 502677417 }]\
"""

expected_rem1 = "[Coin { parent_coin_info: bdae1b280bee66f03004abc111d459b4180bdf864bc500f2f9fe2d0e4d649766, \
puzzle_hash: b5ade02ac0e59f0c74faeb96ba735da1fdf75eab634cd79eed70f507f7fe2b05, \
amount: 21387408051 }, \
Coin { parent_coin_info: e56ccb54fbf9331a8e7188228c604bc0741b63e91789ade4f0f5c70aa6e7a991, \
puzzle_hash: b5ade02ac0e59f0c74faeb96ba735da1fdf75eab634cd79eed70f507f7fe2b05, \
amount: 30322904517 }, \
Coin { parent_coin_info: 00c1969e3a1a688102f661eb55f1865ac13948b57e3aad49f67a47c4aee38aaa, \
puzzle_hash: b5ade02ac0e59f0c74faeb96ba735da1fdf75eab634cd79eed70f507f7fe2b05, \
amount: 24173006931 }, \
Coin { parent_coin_info: e549ed2ddb2e0a7ea87eb061d9d8af37be5fde0e93868c7542ecdd136f229f5a, \
puzzle_hash: b5ade02ac0e59f0c74faeb96ba735da1fdf75eab634cd79eed70f507f7fe2b05, \
amount: 23041511838 }, \
Coin { parent_coin_info: 58bd71303829d3930769330d7a0527b208dba9ed12176b4dee7fd92a580db810, \
puzzle_hash: b5ade02ac0e59f0c74faeb96ba735da1fdf75eab634cd79eed70f507f7fe2b05, \
amount: 20885838908 }, \
Coin { parent_coin_info: 21eda9478e77e503cf75ce8f29dc0df056c2a62848e4d27b20aab6408150c093, \
puzzle_hash: b5ade02ac0e59f0c74faeb96ba735da1fdf75eab634cd79eed70f507f7fe2b05, \
amount: 20979513635 }, \
Coin { parent_coin_info: 652eebdd531e5e18168a15ed875322f0c070ae8f998bd53dd4492c3f0f75042d, \
puzzle_hash: b5ade02ac0e59f0c74faeb96ba735da1fdf75eab634cd79eed70f507f7fe2b05, \
amount: 21105249306 }, \
Coin { parent_coin_info: 0bb45db6ff8a6945599656cfd7d14d500a1fab57fcd9d5ed614d1187deececb1, \
puzzle_hash: a52a857ff8eab99762aaa04c9fb3fbefc6eb6d5e57d30a0da40163c2fc485cf6, \
amount: 2607244231 }]"

expected_add2 = """\
[Coin { parent_coin_info: 3ff28fa9df88131fd1e6eb24ba22506966b0b845236e010b9372486663a0f296, \
puzzle_hash: ebebc91efa8d5f7ed3de7a3c66b6a17abad2208494d053f8ca828c97ec7625c3, \
amount: 252626581182 }, \
Coin { parent_coin_info: b952b249c5e31f3fa676d69580ea54b5a8e002640e3031d6c9fd0d68c2ca2998, \
puzzle_hash: e1c2b1367f94da4cbd9e3a1f7207d56f50aa66efe7c73e28d458dc86511ed070, \
amount: 18668682770000 }, \
Coin { parent_coin_info: b952b249c5e31f3fa676d69580ea54b5a8e002640e3031d6c9fd0d68c2ca2998, \
puzzle_hash: 67eec8df11b2091bdaf35857999f13daeaef9d53a56a32b849c359bc6cbf3be3, \
amount: 19770236820000 }, \
Coin { parent_coin_info: 8e64e64f153df0775030cce5aff409f292b498188145bb2653bb1010e7f6990a, \
puzzle_hash: e8eaf7f1adb9210b8cdcf0935be1ccb1f7f8d441f9c47254a554c37113f298b9, \
amount: 161228019810 }, \
Coin { parent_coin_info: 8e64e64f153df0775030cce5aff409f292b498188145bb2653bb1010e7f6990a, \
puzzle_hash: 4b5b1515c77184350f77092f9be4105a99f76979acdb4aa5cf0fe55604617304, \
amount: 3 }, \
Coin { parent_coin_info: 65a3a0ddb1fb1cb975993af1e5d833e251751f5bcf2c3ddd56b7d90ef1b29961, \
puzzle_hash: a826ff0a92b29a76ae0157734ae4e2b6f537773c1a972ea9ba3dc52c03aacfb9, \
amount: 2800000000 }, \
Coin { parent_coin_info: 65a3a0ddb1fb1cb975993af1e5d833e251751f5bcf2c3ddd56b7d90ef1b29961, \
puzzle_hash: b0251154d08e367d10b18d03888a5bb28a6dcfcf4df93abf3033503bab135e47, \
amount: 1829200000000 }, \
Coin { parent_coin_info: fe863b0be2094196c5a7507f15c29c4f1d8667d10ff1b2c7a2674408328f9e24, \
puzzle_hash: 23a86623d074238c45b4190e4b15c03e40b492744ac7afe32239667121f033ae, \
amount: 62500000000 }, \
Coin { parent_coin_info: fe863b0be2094196c5a7507f15c29c4f1d8667d10ff1b2c7a2674408328f9e24, \
puzzle_hash: 67eec8df11b2091bdaf35857999f13daeaef9d53a56a32b849c359bc6cbf3be3, \
amount: 38376003550000 }, \
Coin { parent_coin_info: 27b6fa7fb5a39590aa04ca072f73b05f77a8e3bd28372f3ec1cfcafb3c31b5af, \
puzzle_hash: 705e7609c1002d14adebfa5f5e75e7a89bf8186f02cf80bd4e1c96bbbc68e5ee, \
amount: 2800000000 }, \
Coin { parent_coin_info: 27b6fa7fb5a39590aa04ca072f73b05f77a8e3bd28372f3ec1cfcafb3c31b5af, \
puzzle_hash: 03ad6280eb95aad016f5fb624e254f10b28b0cd473a7ac41a60e6d10f10ac8ac, \
amount: 1596800000000 }, \
Coin { parent_coin_info: fd0723fa2ed0176528479e17c272bc1f3e28cf3dd5f0c195db69c218b2a64608, \
puzzle_hash: 89560f32833432848770602a6b8ccc098a4b7c14c9e2b08badc9f8a681ee1e5e, \
amount: 1 }, \
Coin { parent_coin_info: fd0723fa2ed0176528479e17c272bc1f3e28cf3dd5f0c195db69c218b2a64608, \
puzzle_hash: 6bde1e0c6f9d3b93dc5e7e878723257ede573deeed59e3b4a90f5c86de1a0bd3, \
amount: 1750000000000 }, \
Coin { parent_coin_info: 0bf45d00957451c7c454ec7232550547cf27ce4da1c0c5724d98067d123aa7b2, \
puzzle_hash: e2bc9da031d971a50eacb2c8b727de3f41975f9543ba23dd24a6390b99e09033, \
amount: 5600000000 }, \
Coin { parent_coin_info: 0bf45d00957451c7c454ec7232550547cf27ce4da1c0c5724d98067d123aa7b2, \
puzzle_hash: dabfb22f8fd1c2ca4b35d24cc51f8557c61c0a6825866b75c3b0cce9b7c4c6ce, \
amount: 1226900000000 }]\
"""

expected_rem2 = "[Coin { parent_coin_info: c8c3956842aac8270c3a42fd5eaa8cfc0cf97c2149ac87ac422b976284965646, \
puzzle_hash: 0598413a497cdbb247506965b0bc14f8160ec7570014f2e962464efea71ff2cd, \
amount: 29080250000 }, \
Coin { parent_coin_info: c8c3956842aac8270c3a42fd5eaa8cfc0cf97c2149ac87ac422b976284965646, \
puzzle_hash: 5e5c7ebdb1d60fdaf082589114b96e592b15405a848b548f31428ef1b6df2541, \
amount: 29074490000 }, \
Coin { parent_coin_info: dbf3d3dde23c3b34fad7512154d2297286592a1a0ea6e95a7ca4ed68fcd72d51, \
puzzle_hash: 5d7b7a8a6d5e2e40ab87012e9e81b2c96e83b3b36c1089e74421f7c5f882af91, \
amount: 29000000000 }, \
Coin { parent_coin_info: 5beefdbcc7c1955a0ce7ed68595f6815388e80d1689a2b56813fadd47446461e, \
puzzle_hash: ef6f97f3cf50b5c7abb8887d4a39c40f0b9cc6369c0eeb1ec8bfca8459f1158d, \
amount: 28843320000 }, \
Coin { parent_coin_info: 6e314d8719e320e1459934b488de296138677e12049317d2a6769fa12f14cb4e, \
puzzle_hash: 02d30e3dbc75b0153f7f9bd2f30131e5e73163f2c75540ee1389c368f96e9726, \
amount: 28741360000 }, \
Coin { parent_coin_info: fa010ef1b3fa32ce3627b5f72d57f2a6f4b9cd22b677e4092d1503032b8802ea, \
puzzle_hash: 8e0ad114f402d7f264c22d7e35867acf243426741689afa0de558e2102487cf4, \
amount: 28320000000 }, \
Coin { parent_coin_info: af4f67c4727e550fb7ec15132892d8ef72fd2bb9b450f7924f88b39c4c4e1dde, \
puzzle_hash: 5641d9521eaa987f6ec869118dcc15b1ae5df4eaa6cef4287c10ac803c21d1c2, \
amount: 28178570000 }, \
Coin { parent_coin_info: c01662f33684b400d3ce9f4a94f88f3595209b24860ef06c678c3a64749465ec, \
puzzle_hash: a7c7c5defce0512deb0b9017bde7f54584ae0169891530c513c24a931c201325, \
amount: 28112990000 }, \
Coin { parent_coin_info: 8da6be96a7dea0d9ca02e59fa5422af304f9639087aaa4c3533853ec8e18bb32, \
puzzle_hash: 68b46673c7381bf849b3cc749f370dcd0377e718e19412b87b7c83c1303f9f17, \
amount: 11738857599 }, \
Coin { parent_coin_info: 8da6be96a7dea0d9ca02e59fa5422af304f9639087aaa4c3533853ec8e18bb32, \
puzzle_hash: bf0c89aed73aaaf419b7ebadbc109df2692def5f9ddd6a88cc65e6e177d26619, \
amount: 11536743593 }, \
Coin { parent_coin_info: 1fc23a2ec3e69536c1d61d976eac844a671fc64eb1d48ebe6c0e8d430bcc6d36, \
puzzle_hash: 67eec8df11b2091bdaf35857999f13daeaef9d53a56a32b849c359bc6cbf3be3, \
amount: 38438919590000 }, \
Coin { parent_coin_info: bf83e0b57b06c374ebf9f3dc6fc54273ffa6ac8542869c982f29c39352d37274, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10007057187 }, \
Coin { parent_coin_info: e5adec9be534f80f554faf079984457ff967c6533976e27af47f274524ee5523, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10043722666 }, \
Coin { parent_coin_info: 742a4730a261782c0c084265efeca8107939e6f514482e109ada0f799e72482f, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10007835305 }, \
Coin { parent_coin_info: 9b44f49c7719ba16ce1222eb4aaef51b81c275891e10eab7863f7445cc2407b3, \
puzzle_hash: be245cfcb01ade8447280984bb0921dac3586d9134e48097bb356ae4f180c4f5, \
amount: 911411542 }, \
Coin { parent_coin_info: e5766c13260a277166fd0016c62558cd5126d8d5a2bdaff91294b992e8565fcc, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10021424728 }, \
Coin { parent_coin_info: 309bbbe268ef10b5a71c770874feb803f5ad3feace5d2720d7aacd7ce56b50f1, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10021136962 }, \
Coin { parent_coin_info: d5f743c16269a9bd9001484cadf1492307d6909cb0fc57cf8bb33dcfcb1f9ea0, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10030757856 }, \
Coin { parent_coin_info: d94037be72021d935e7bca6a7dc5094306f7e0663520ace0a6eb54c7ab45d07e, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10013674801 }, \
Coin { parent_coin_info: a007b52a08394fe6f6c93a9ebdf86d1a2f5cd47316610e267da625f7a252505c, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10017528580 }, \
Coin { parent_coin_info: 060993df4b0a329554104f94e4e5902fde0c4522d7a090184c83d4d0ce0f1b2d, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10013130057 }, \
Coin { parent_coin_info: cbaf4e8e557bcf08391e294b5f69912ce2b3405a663906776322e968f6c46863, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10009177123 }, \
Coin { parent_coin_info: acfd3b378448bad472df76a9abf5a03731e21a44d3fe63a47443bc1fa79e4c5c, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10003050807 }, \
Coin { parent_coin_info: cb361e8e15de37b4bfcf6dbba0977c080c2a42239d587ed24cdfa73b49a50f67, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10030007305 }, \
Coin { parent_coin_info: 5510782c5870b7fe8a6538c30f524a9d23583a471d663cd865369f058d237b06, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10008269397 }, \
Coin { parent_coin_info: ea407f8ce042079b20e89005ed848595d9e4801d9c4f9c16e8cecc136e8ba984, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10028055078 }, \
Coin { parent_coin_info: 2c2178f3d68ad25f15dd09b9826f3c8a492339ed5b3110fb51e57f1b1c7ff0a6, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10040043042 }, \
Coin { parent_coin_info: 7ddde8745d54aaebcbb2df2bee42e5113f794127a926c031a8594313e6f74f70, \
puzzle_hash: 0ce7216b7dcbdc234d4197fcf8fe252b3393ed02411d6bc8c441ad352450f2bb, \
amount: 10021737377 }, \
Coin { parent_coin_info: a2979840c7f7a3f732ec02cfdb1f9f0a8aa53d70ceb19457c00e1665b020e74b, \
puzzle_hash: 189f799c1d472a4dc7b12ea9316649cbea83c53b26aaa764ec504cf8fe992c07, \
amount: 1832000000000 }, \
Coin { parent_coin_info: eb011cebe504e5342b9ec4d4a1ecfb51f53054f79f324b4f1fcc571f82c9e2bb, \
puzzle_hash: 67eec8df11b2091bdaf35857999f13daeaef9d53a56a32b849c359bc6cbf3be3, \
amount: 38438503550000 }, \
Coin { parent_coin_info: 17893cd92eb6a01efc8344792b69313c0cf685359093e68b95285bbc0ffe16b9, \
puzzle_hash: cb715b7241eef620cb437066496d149c25df72863701540c1e6058ce41318346, \
amount: 1599600000000 }, \
Coin { parent_coin_info: ae141dd0538f16353c2d5d1bb0e5b24116771bda5644c9d51af40d5fc7271628, \
puzzle_hash: 89560f32833432848770602a6b8ccc098a4b7c14c9e2b08badc9f8a681ee1e5e, \
amount: 1 }, \
Coin { parent_coin_info: ccd5bb71183532bff220ba46c268991a000000000000000000000000000f429f, \
puzzle_hash: afc39545db15ba93520cc1ec0a3f087376f7efcd9feeb6f04adcd4b1f4b4d68d, \
amount: 1750000000000 }, \
Coin { parent_coin_info: 7eda8c4f902fbfbdd28b3e0381c8415f4d42df816ff12432b0f1c7d0fdda9ba0, \
puzzle_hash: 25192af3f69f3d2c347f97b31925f406372e35136dbe40dda6230750a6eaa1d2, \
amount: 1232500000000 }]"

@pytest.mark.parametrize(
    "ty", [SpendBundle, PySpendBundle]
)
@pytest.mark.parametrize(
    "input_file, expected_add, expected_rem",
    [
        ("3000253", expected_add1, expected_rem1),
        ("1000101", expected_add2, expected_rem2),
    ],
)
def test_spend_bundle(ty: Type, input_file: str, expected_add: str, expected_rem: str) -> None:
    buf = open(f"test-bundles/{input_file}.bundle", "rb").read()
    bundle = ty.from_bytes(buf)

    additions = bundle.additions()
    removals = bundle.removals()

    add = f"{additions}"
    assert add == expected_add

    rem = f"{removals}"
    assert rem == expected_rem
