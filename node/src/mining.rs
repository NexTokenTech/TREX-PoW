use sp_core::U256;
use sc_network::config::NodeKeyConfig;
use log::info;
use sc_service::Error;

pub fn generate_mining_seed(
    node_key: NodeKeyConfig,
) -> Result<U256, Error>{
    let local_identity = node_key.into_keypair()?;
    let local_public = local_identity.public();
    let local_peer_id = local_public.clone().to_peer_id();
    let mut local_peer_vec = local_peer_id.to_bytes();

    let mut number = 0;
    let max_number = local_peer_vec.len()/2;
    while number < max_number {
        local_peer_vec.pop();
        number += 1;
    }
    let mining_key = U256::from_little_endian(&local_peer_vec);
    // TODO: if the runtime info problem is resolved, this part of code is no longer necessary.
    info!(
			target: "sub-libp2p",
			"🏷  Local node identity is: {}",
			local_peer_id.to_base58(),
		);
    Ok(mining_key)
}