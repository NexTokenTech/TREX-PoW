// tag::description[]
//! Generic implementations of Extrinsic/Header/Block.
// end::description[]

mod block;
mod checked_extrinsic;
mod unchecked_extrinsic;
mod header;

pub use self::{
    block::{Block, BlockId, SignedBlock},
    checked_extrinsic::CheckedExtrinsic,
    header::Header,
    unchecked_extrinsic::{SignedPayload, UncheckedExtrinsic},
};
