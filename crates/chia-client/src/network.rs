use chia_protocol::Bytes32;
use hex_literal::hex;

#[derive(Debug, Clone)]
pub struct Network {
    pub network_id: String,
    pub default_port: u16,
    pub genesis_challenge: Bytes32,
    pub dns_introducers: Vec<String>,
}

impl Network {
    pub fn mainnet() -> Self {
        Self {
            network_id: "mainnet".to_string(),
            default_port: 8444,
            genesis_challenge: Bytes32::new(hex!(
                "ccd5bb71183532bff220ba46c268991a3ff07eb358e8255a65c30a2dce0e5fbb"
            )),
            dns_introducers: vec![
                "dns-introducer.chia.net".to_string(),
                "chia.ctrlaltdel.ch".to_string(),
                "seeder.dexie.space".to_string(),
                "chia.hoffmang.com".to_string(),
            ],
        }
    }

    pub fn testnet11() -> Self {
        Self {
            network_id: "testnet11".to_string(),
            default_port: 58444,
            genesis_challenge: Bytes32::new(hex!(
                "37a90eb5185a9c4439a91ddc98bbadce7b4feba060d50116a067de66bf236615"
            )),
            dns_introducers: vec!["dns-introducer-testnet11.chia.net".to_string()],
        }
    }
}
