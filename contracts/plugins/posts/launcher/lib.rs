#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[openbrush::implementation(Ownable, Upgradeable)]
#[openbrush::contract]
mod posts_launcher {
  use ink::ToAccountId;
  use ink::env::call::{build_create, ExecutionInput, Selector};
  use ink::storage::{Mapping, Lazy};
  use ink::prelude::string::String;
  use scale::{Decode, Encode};
  use posts::{PostsRef};
  use openbrush::{modifiers, traits::Storage};
  use shared::ensure;
  use shared::traits::codehash::*;

  #[derive(Encode, Decode, Debug, PartialEq, Eq)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum LauncherError {
    Custom(String),
    OwnableError(OwnableError),
    UnAuthorized,
  }

  impl From<OwnableError> for LauncherError {
    fn from(error: OwnableError) -> Self {
      LauncherError::OwnableError(error)
    }
  }

  type LauncherResult<T> = core::result::Result<T, LauncherError>;
  type Version = u32;
  type Nonce = u32;

  #[ink(storage)]
  #[derive(Default, Storage)]
  pub struct PostsLauncher {
    motherspace_id: Lazy<AccountId>,

    plugin_codes_nonce: Lazy<Nonce>,
    plugin_codes: Mapping<Version, Hash>,

    launches_count: Lazy<u32>,
    #[storage_field]
    ownable: ownable::Data,
  }

  impl CodeHash for PostsLauncher {}

  impl PostsLauncher {
    #[ink(constructor)]
    pub fn new(motherspace_id: AccountId, owner_id: AccountId, plugin_code: Hash) -> Self {
      let mut one = PostsLauncher::default();
      one.motherspace_id.set(&motherspace_id);
      one.upgrade_plugin_code_impl(plugin_code);
      ownable::Internal::_init_with_owner(&mut one, owner_id);

      one
    }

    #[ink(message)]
    pub fn latest_plugin_code(&self) -> Hash {
      self.latest_plugin_code_impl()
    }

    /// For now, we can only upgrade plugin code via motherspace
    #[ink(message)]
    pub fn upgrade_plugin_code(&mut self, new_code_hash: Hash) -> LauncherResult<Version> {
      self.ensure_motherspace()?;
      Ok(self.upgrade_plugin_code_impl(new_code_hash))
    }

    #[ink(message)]
    pub fn launch(&mut self, space_id: AccountId) -> LauncherResult<AccountId> {
      let launcher_id = Self::env().account_id();

      let next_launches_count =
        self.launches_count.get_or_default()
          .checked_add(1)
          .expect("Exceeds number of launch count!");

      let input =
        ExecutionInput::new(Selector::new(ink::selector_bytes!("new")))
          .push_arg(space_id)
          .push_arg(launcher_id);

      let new_contract: PostsRef = build_create::<PostsRef>()
        .code_hash(self.latest_plugin_code())
        .gas_limit(0)
        .endowment(0)
        .exec_input(input)
        .salt_bytes(next_launches_count.to_le_bytes())
        .returns::<PostsRef>()
        .instantiate();

      self.launches_count.set(&next_launches_count);

      Ok(new_contract.to_account_id())
    }

    #[ink(message)]
    pub fn launches_count(&self) -> u32 {
      self.launches_count.get_or_default()
    }

    #[ink(message)]
    pub fn motherspace_id(&self) -> AccountId {
      self.motherspace_id.get().unwrap()
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

    fn ensure_motherspace(&self) -> LauncherResult<()> {
      ensure!(self.motherspace_id() == self.env().caller(), LauncherError::UnAuthorized);
      Ok(())
    }
  }
}
