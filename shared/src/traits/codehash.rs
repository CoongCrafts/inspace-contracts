use openbrush::traits::DefaultEnv;

#[openbrush::trait_definition]
pub trait CodeHash: Sized {
  #[ink(message)]
  fn code_hash(&self) -> ::ink::primitives::Hash {
    Self::env().code_hash(&Self::env().account_id()).unwrap()
  }
}