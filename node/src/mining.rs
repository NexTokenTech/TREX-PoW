use sp_core::U256;
use sc_network::config::NodeKeyConfig;
use log::info;
use sc_service::Error;

pub fn generate_mining_seed(
    node_key: NodeKeyConfig,
) -> Result<U256, Error>{
    // convert node_key to keypair
    let local_identity = node_key.into_keypair()?;
    // convert keypair to PublicKey
    let local_public = local_identity.public();
    // convert PublicKey to PeerId
    let local_peer_id = local_public.clone().to_peer_id();
    // convert PeerId to Vec<u8>
    let mut local_peer_vec = local_peer_id.to_bytes();

    // delete count
    let mut number = 0;
    // The maximum length of U256::from_little_endian slice is 32,If it exceeds the number of digits,an error will be reported 'assertion failed: 4 * 8 >= slice.len()'.So we currently intercept the 32 length of local_peer_vec.
    let max_number = local_peer_vec.len()-32;
    // Remove Vector elements other than 32 bits
    while number < max_number {
        local_peer_vec.pop();
        number += 1;
    }
    // Convert the intercepted 32 length Vec<u8> to U256 format
    let mining_key = U256::from_little_endian(&local_peer_vec);
    // TODO: if the runtime info problem is resolved, this part of code is no longer necessary.
    info!(
			target: "sub-libp2p",
			"🏷  Local node identity is: {}",
			local_peer_id.to_base58(),
		);
    // Return Result containing mining_key
    Ok(mining_key)
}