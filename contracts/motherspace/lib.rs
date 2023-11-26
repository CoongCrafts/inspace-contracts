#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod motherspace {
  use traits::{Upgradeable};
  use ink::storage::Lazy;
  use ink::prelude::string::String;
  use helper_macros::*;

  type Result<T> = core::result::Result<T, Error>;

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub enum Error {
    Custom(String),
  }


  #[ink(storage)]
  #[derive(Default)]
  pub struct MotherSpace {
    value: bool,
    spaces_count: u32,

    owner_id: Lazy<AccountId>,
  }

  impl MotherSpace {
    /// Constructor that initializes the `bool` value to the given `init_value`.
    #[ink(constructor)]
    pub fn new(init_value: bool) -> Self {
      Self {
        value: init_value,
        ..Default::default()
      }
    }

    /// A message that can be called on instantiated contracts.
    /// This one flips the value of the stored `bool` from `true`
    /// to `false` and vice versa.
    #[ink(message)]
    pub fn flip(&mut self) {
      self.value = !self.value;
    }

    /// Simply returns the current value of our `bool`.
    #[ink(message)]
    pub fn get(&self) -> bool {
      self.value
    }

    #[ink(message)]
    pub fn deploy(&mut self) -> Result<()> {
      self.spaces_count = self.spaces_count.checked_add(1).unwrap();
      Ok(())
    }

    #[ink(message)]
    pub fn spaces_count(&self) -> u32 {
      self.spaces_count
    }
  }

  impl Upgradeable for MotherSpace {
    #[ink(message)]
    fn set_code_hash(&mut self, code_hash: Hash) {
      // TODO ensure owner!
      ::ink::env::set_code_hash2::<Environment>(&code_hash).unwrap_or_else(|err| {
        panic!(
          "Failed to `set_code_hash` to {:?} due to {:?}",
          code_hash, err
        )
      });
      ::ink::env::debug_println!("Switched code hash to {:?}.", code_hash);
    }

    #[ink(message)]
    fn code_hash(&self) -> Hash {
      self.env().code_hash(&self.env().account_id()).unwrap()
    }
  }
}
