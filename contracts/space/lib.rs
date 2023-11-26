#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod space {
  use ink::storage::Lazy;
  use ink::prelude::string::String;
  use helper_macros::*;

  type Result<T> = core::result::Result<T, Error>;

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub enum Error {
    Custom(String)
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct SpaceInfo {
    name: String,
    desc: Option<String>,
  }

  #[derive(Clone, Debug, Copy, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct SpaceOwnable {
    motherspace_id: AccountId,
    owner_id: AccountId,
  }

  #[ink(storage)]
  #[derive(Default)]
  pub struct Space {
    info: Lazy<SpaceInfo>,
    ownable: Lazy<SpaceOwnable>,
  }

  impl Space {
    #[ink(constructor)]
    pub fn new(motherspace_id: AccountId, owner_id: AccountId, space_info: SpaceInfo) -> Result<Self> {
      ensure!(motherspace_id == Self::env().caller(), Error::Custom(String::from("Only MotherSpace can deploy spaces!")));
      ensure!(space_info.name.len() <= 30, Error::Custom(String::from("Space name is at max 30 chars")));
      ensure!(space_info.name.len() >= 3, Error::Custom(String::from("Space name must be at least 3 chars")));

      if let Some(desc) = space_info.desc.clone() {
        ensure!(desc.len() <= 100, Error::Custom(String::from("Space name is at max 100 chars")));
      }

      let mut instance = Space::default();
      instance.info.set(&space_info);

      let ownable = SpaceOwnable { motherspace_id, owner_id };
      instance.ownable.set(&ownable);

      Ok(instance)
    }

    #[ink(message)]
    pub fn owner_id(&self) -> AccountId {
      self.ownable.get().unwrap().owner_id
    }

    #[ink(message)]
    pub fn motherspace_id(&self) -> AccountId {
      self.ownable.get().unwrap().motherspace_id
    }

    #[ink(message)]
    pub fn info(&self) -> SpaceInfo {
      self.info.get().unwrap()
    }
  }
}
