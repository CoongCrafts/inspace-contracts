#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod flipper_launcher {
  use ink::ToAccountId;
  use ink::env::call::{build_create, ExecutionInput, Selector};
  use ink::storage::{Mapping, Lazy};
  use ink::prelude::string::String;
  use scale::{Decode, Encode};
  use flipper::{FlipperRef};
  use helper_macros::*;

  #[derive(Encode, Decode, Debug, PartialEq, Eq, Clone)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum Error {
    Custom(String),
    UnAuthorized,
  }

  type Result<T> = core::result::Result<T, Error>;
  type Version = u32;
  type Nonce = u32;

  #[ink(storage)]
  #[derive(Default)]
  pub struct FlipperLauncher {
    motherspace_id: Lazy<AccountId>,
    owner_id: Lazy<AccountId>,

    plugin_codes_nonce: Lazy<Nonce>,
    plugin_codes: Mapping<Version, Hash>,

    launches_count: Lazy<u32>,
  }

  impl FlipperLauncher {
    #[ink(constructor)]
    pub fn new(motherspace_id: AccountId, owner_id: AccountId, plugin_code: Hash) -> Self {
      let mut one = FlipperLauncher::default();
      one.motherspace_id.set(&motherspace_id);
      one.owner_id.set(&owner_id);

      one.upgrade_plugin_code_impl(plugin_code);

      one
    }

    #[ink(message)]
    pub fn latest_plugin_code(&self) -> Hash {
      self.latest_plugin_code_impl()
    }

    #[ink(message)]
    pub fn upgrade_plugin_code(&mut self, new_code_hash: Hash) -> Result<Version> {
      Ok(self.upgrade_plugin_code_impl(new_code_hash))
    }

    #[ink(message)]
    pub fn launch(&mut self, space_id: AccountId) -> Result<AccountId> {
      let launcher_id = Self::env().account_id();

      let next_launches_count =
        self.launches_count.get_or_default()
          .checked_add(1)
          .expect("Exceeds number of launch count!");

      let input =
        ExecutionInput::new(Selector::new(ink::selector_bytes!("new")))
          .push_arg(space_id)
          .push_arg(launcher_id);

      let new_contract: FlipperRef = build_create::<FlipperRef>()
        .code_hash(self.latest_plugin_code())
        .gas_limit(0)
        .endowment(0)
        .exec_input(input)
        .salt_bytes(next_launches_count.to_le_bytes())
        .returns::<FlipperRef>()
        .instantiate();

      self.launches_count.set(&next_launches_count);

      Ok(new_contract.to_account_id())
    }

    // #[ink(message)]
    // pub fn update_motherspace_id(&mut self, new_motherspace_id: AccountId) -> Result<()> {
    //   ensure!(self.owner_id() == self.env().caller(), Error::UnAuthorized);
    //
    //   self.motherspace_id.set(&new_motherspace_id);
    //
    //   Ok(())
    // }

    #[ink(message)]
    pub fn launches_count(&self) -> u32 {
      self.launches_count.get_or_default()
    }

    #[ink(message)]
    pub fn motherspace_id(&self) -> AccountId {
      self.motherspace_id.get().unwrap()
    }

    #[ink(message)]
    pub fn owner_id(&self) -> AccountId {
      self.owner_id.get().unwrap()
    }

    fn upgrade_plugin_code_impl(&mut self, new_plugin_code: Hash) -> Version {
      let next_plugin_code_version: Version = self.plugin_codes_nonce.get_or_default().checked_add(1).expect("Exceeds number ");
      self.plugin_codes.insert(next_plugin_code_version, &new_plugin_code);
      self.plugin_codes_nonce.set(&next_plugin_code_version);

      next_plugin_code_version
    }

    fn latest_plugin_code_impl(&self) -> Hash {
      self.plugin_codes.get(self.plugin_codes_nonce.get_or_default()).unwrap()
    }
  }
}
