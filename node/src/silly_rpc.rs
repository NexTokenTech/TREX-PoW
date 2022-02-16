use jsonrpc_core::{IoHandler, Result};
use jsonrpc_derive::rpc;

#[rpc]
pub trait SillyRpc {
    #[rpc(name = "hello_rpc")]
    fn hello_rpc(&self) -> Result<String>;

    #[rpc(name = "silly_seven")]
    fn silly_7(&self) -> Result<u64>;

    #[rpc(name = "silly_double")]
    fn silly_double(&self, val: u64) -> Result<u64>;
}

pub struct Silly;

impl SillyRpc for Silly {
    fn hello_rpc(&self) -> Result<String>{
        Ok("Hello World,Rpc".into())
    }

    fn silly_7(&self) -> Result<u64> {
        Ok(7)
    }

    fn silly_double(&self, val: u64) -> Result<u64> {
        Ok(2 * val)
    }
}