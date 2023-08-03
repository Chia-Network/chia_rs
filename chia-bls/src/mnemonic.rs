use anyhow::Error;
use bip39::{Language, Mnemonic, Seed};
use std::array::TryFromSliceError;
use std::result::Result;

pub fn entropy_to_mnemonic(entropy: &[u8; 32]) -> String {
    Mnemonic::from_entropy(entropy, Language::English)
        .unwrap()
        .into_phrase()
}

pub fn mnemonic_to_entropy(mnemonic: &str) -> Result<[u8; 32], Error> {
    let m = Mnemonic::from_phrase(mnemonic, Language::English)?;
    let ent = m.entropy();
    ent.try_into().map_err(|e: TryFromSliceError| {
        Error::from(e).context("incorrect number of words in mnemonic")
    })
}

pub fn entropy_to_seed(entropy: &[u8; 32]) -> [u8; 64] {
    let m = Mnemonic::from_entropy(entropy, Language::English).unwrap();
    Seed::new(&m, "").as_bytes().try_into().unwrap()
}

#[cfg(test)]
use hex::FromHex;

#[test]
fn test_parse_mnemonic() {
    // test vectors from BIP39
    // https://github.com/trezor/python-mnemonic/blob/master/vectors.json
    // The seeds are changed to account for chia using an empty passphrase
    // (whereas the trezor test vectors use "TREZOR")

    // phrase, entropy, seed
    let test_cases = &[
        ("all hour make first leader extend hole alien behind guard gospel lava path output census museum junior mass reopen famous sing advance salt reform",
        "066dca1a2bb7e8a1db2832148ce9933eea0f3ac9548d793112d9a95c9407efad",
        "fc795be0c3f18c50dddb34e72179dc597d64055497ecc1e69e2e56a5409651bc139aae8070d4df0ea14d8d2a518a9a00bb1cc6e92e053fe34051f6821df9164c"
        ),
        ("void come effort suffer camp survey warrior heavy shoot primary clutch crush open amazing screen patrol group space point ten exist slush involve unfold",
        "f585c11aec520db57dd353c69554b21a89b20fb0650966fa0a9d6f74fd989d8f",
        "b873212f885ccffbf4692afcb84bc2e55886de2dfa07d90f5c3c239abc31c0a6ce047e30fd8bf6a281e71389aa82d73df74c7bbfb3b06b4639a5cee775cccd3c"
        ),
        ("panda eyebrow bullet gorilla call smoke muffin taste mesh discover soft ostrich alcohol speed nation flash devote level hobby quick inner drive ghost inside",
        "9f6a2878b2520799a44ef18bc7df394e7061a224d2c33cd015b157d746869863",
        "3e066d7dee2dbf8fcd3fe240a3975658ca118a8f6f4ca81cf99104944604b05a5090a79d99e545704b914ca0397fedb82fd00fd6a72098703709c891a065ee49")
    ];

    for (phrase, entropy, seed) in test_cases {
        println!("{}", phrase);
        assert_eq!(
            hex::encode(mnemonic_to_entropy(phrase).unwrap()),
            entropy.to_string()
        );
        assert_eq!(
            entropy_to_mnemonic(&<[u8; 32]>::from_hex(entropy).unwrap()).as_str(),
            *phrase
        );
        assert_eq!(
            hex::encode(entropy_to_seed(&<[u8; 32]>::from_hex(entropy).unwrap())),
            seed.to_string()
        )
    }
}

#[test]
fn test_invalid_mnemonic() {
    assert_eq!(
        format!(
            "{}",
            mnemonic_to_entropy("camp survey warrior").unwrap_err()
        ),
        "invalid number of words in phrase: 3"
    );
    assert_eq!(
        format!(
            "{}",
            mnemonic_to_entropy(
                "panda eyebrow bullet gorilla call smoke muffin taste mesh discover soft ostrich"
            )
            .unwrap_err()
        ),
        "invalid checksum"
    );
    assert_eq!(format!("{}", mnemonic_to_entropy("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap_err()), "incorrect number of words in mnemonic");
    assert_eq!(mnemonic_to_entropy("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art").unwrap(), <[u8; 32]>::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap());

    assert_eq!(mnemonic_to_entropy("letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless").unwrap(),
        <[u8; 32]>::from_hex("8080808080808080808080808080808080808080808080808080808080808080").unwrap());

    // make sure all whitespace is ignored
    assert_eq!(mnemonic_to_entropy("letter       advice  \t cage\t absurd \tamount doctor acoustic \n avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless").unwrap(),
        <[u8; 32]>::from_hex("8080808080808080808080808080808080808080808080808080808080808080").unwrap());
}
