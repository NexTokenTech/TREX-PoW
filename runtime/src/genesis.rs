use super::{AccountId, BalancesConfig, DifficultyModuleConfig, GenesisConfig, Signature, SudoConfig, SystemConfig, TransactionPaymentConfig};
use sp_core::{sr25519, Pair};
use sp_runtime::traits::{IdentifyAccount, Verify};
use cp_constants::{
    MIN_DIFFICULTY
};

/// Helper function to generate a crypto pair from seed
fn get_from_seed<TPair: Pair>(seed: &str) -> TPair::Public {
    TPair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn account_id_from_seed<TPair: Pair>(seed: &str) -> AccountId
    where
        AccountPublic: From<TPair::Public>,
{
    AccountPublic::from(get_from_seed::<TPair>(seed)).into_account()
}

pub fn dev_genesis(wasm_binary: &[u8]) -> GenesisConfig {
    testnet_genesis(
        wasm_binary,
        // Root Key
        account_id_from_seed::<sr25519::Pair>("Alice"),
        // Endowed Accounts
        vec![
            account_id_from_seed::<sr25519::Pair>("Alice"),
            account_id_from_seed::<sr25519::Pair>("Bob"),
            account_id_from_seed::<sr25519::Pair>("Alice//stash"),
            account_id_from_seed::<sr25519::Pair>("Bob//stash"),
        ],
    )
}

/// Helper function to build a genesis configuration
pub fn testnet_genesis(
    wasm_binary: &[u8],
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
) -> GenesisConfig {
    GenesisConfig {
        system: SystemConfig {
            code: wasm_binary.to_vec()
        },
        difficulty_module: DifficultyModuleConfig {
            initial_difficulty: MIN_DIFFICULTY
        },
        balances: BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1 << 60))
                .collect(),
        },
        sudo: SudoConfig { key: Some(root_key) },
        transaction_payment: TransactionPaymentConfig {}
    }
}