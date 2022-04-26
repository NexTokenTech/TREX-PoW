use elgamal_capsule::elgamal::PublicKey;
use codec::{Decode, Encode};

#[derive(Debug, Clone)]
pub struct MappingError;

// type alias for mapping result.
pub type MapResult<T> = std::result::Result<T, MappingError>;

/// type alias for solutions of pollard rho method.
pub type Solutions<I> = (Solution<I>, Solution<I>);

/// Solution within pollard rho method.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct Solution<I> {
    pub a: I,
    pub b: I,
    pub n: I,
}

/// Node state in the cycle finding algorithm.
#[derive(Debug, Clone)]
pub struct State<I> {
    pub solution: Solution<I>,
    // y_i in last step
    pub nonce: I,
    // current y_i
    pub work: I,
    pub pubkey: PublicKey,
}

pub trait Hash<I, E: Encode> {
    fn set_nonce(&mut self, int: &I);
    fn get_nonce(&self) -> E;
    fn hash_integer(&self) -> I;
}

/// Mapping nodes in the DAG generated in Pollard Rho method.
pub trait Mapping<I> {
    /// This function represents: x_(i+1) = func_f(x_i)
    fn func_f(&self, x_i: &I, y_i: &I) -> MapResult<I>;
    /// This function represents: a_(i+1) = func_g(a_i, x_i)
    fn func_g(&self, a_i: &I, x_i: &I) -> MapResult<I>;
    /// This function represents: b_(i+1) = func_g(b_i, x_i)
    fn func_h(&self, b_i: &I, x_i: &I) -> MapResult<I>;
}

pub trait CycleFinding<I, E: Encode>: Mapping<I> {
    /// Use current state and block hash to find next state.
    fn transit<C: Hash<I, E>>(self, compute: &mut C) -> MapResult<State<I>>;
}

/// Solver trait to generate private key from intermediate solution in pollard rho method.
pub trait KeySolver<I> {
    /// Solve the private key.
    fn solve(&self) -> Option<I>;
}
