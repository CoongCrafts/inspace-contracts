/// Ref: https://use.ink/basics/upgradeable-contracts#properties-1
#[ink::trait_definition]
pub trait Upgradeable {
  #[ink(message)]
  fn set_code_hash(&mut self, code_hash: ::ink::primitives::Hash);

  #[ink(message)]
  fn code_hash(&self) -> ::ink::primitives::Hash;
}
