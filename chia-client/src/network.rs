#[derive(Debug, Clone)]
pub struct Network {
    pub network_id: String,
    pub agg_sig_me_extra_data: [u8; 32],
}
