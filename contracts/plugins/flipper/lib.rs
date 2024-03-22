#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use flipper::{FlipperRef};

#[openbrush::contract]
mod flipper {
  use openbrush::{modifiers, traits::Storage};
  use shared::traits::codehash::*;
  use shared::traits::plugin_base::*;

  #[ink(storage)]
  #[derive(Default, Storage)]
  pub struct Flipper {
    #[storage_field]
    base: plugin_base::Data,

    value: bool
  }

  impl CodeHash for Flipper {}
  impl PluginBase for Flipper {}

  impl Flipper {
    #[ink(constructor)]
    pub fn new(space_id: AccountId, launcher_id: AccountId) -> Self {
      let mut one = Self::default();
      plugin_base::PluginBase::_init(&mut one, space_id, launcher_id);

      one
    }

    /// Flips the current value of the Flipper's boolean.
    /// Only active member can flip
    #[ink(message)]
    #[modifiers(only_active_member)]
    pub fn flip(&mut self) -> PluginResult<()> {
      self.value = !self.value;

      Ok(())
    }

    /// Returns the current value of the Flipper's boolean.
    #[ink(message)]
    pub fn get(&self) -> bool {
      self.value
    }
  }
}
