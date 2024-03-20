use ink::prelude::string::String;
use openbrush::{
  modifiers,
  traits::{
    AccountId,
    Storage,
    Hash
  },
  storage::{Mapping},
  contracts::{ownable::OwnableError}
};
use crate::ensure;
pub use crate::traits::plugin_launcher;

pub type Version = u32;
pub type Nonce = u32;

#[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
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

#[derive(Default, Debug)]
#[openbrush::storage_item]
pub struct Data {
  #[lazy]
  pub motherspace_id: AccountId,

  #[lazy]
  pub plugin_codes_nonce: Nonce,
  pub plugin_codes: Mapping<Version, Hash>,

  #[lazy]
  pub launches_count: u32,
}

#[openbrush::trait_definition]
pub trait PluginLauncher: Storage<Data> + Instantiator {
  #[ink(message)]
  fn latest_plugin_code(&self) -> Hash {
    self._latest_plugin_code()
  }

  #[ink(message)]
  fn upgrade_plugin_code(&mut self, new_code_hash: Hash) -> Result<Version, LauncherError> {
    // For now, we can only upgrade plugin code via motherspace
    self._ensure_motherspace()?;
    Ok(self._upgrade_plugin_code(new_code_hash))
  }

  #[ink(message)]
  fn launches_count(&self) -> u32 {
    self.data().launches_count.get_or_default()
  }

  #[ink(message)]
  fn motherspace_id(&self) -> AccountId {
    self.data().motherspace_id.get().unwrap()
  }

  #[ink(message)]
  fn launch(&mut self, space_id: AccountId) -> Result<AccountId, LauncherError> {
    let launcher_id = Self::env().account_id();

    let next_launches_count =
      self.data().launches_count.get_or_default()
        .checked_add(1)
        .expect("Exceeds number of launch count!");

    let salt = next_launches_count.to_le_bytes();
    let new_contract_id = self._initiate_new_plugin(space_id, launcher_id, &salt)?;

    self.data().launches_count.set(&next_launches_count);

    Ok(new_contract_id)
  }

  fn _upgrade_plugin_code(&mut self, new_plugin_code: Hash) -> Version {
    let next_plugin_code_version: Version = self.data().plugin_codes_nonce.get_or_default().checked_add(1).expect("Exceeds number ");
    self.data().plugin_codes.insert(&next_plugin_code_version, &new_plugin_code);
    self.data().plugin_codes_nonce.set(&next_plugin_code_version);

    next_plugin_code_version
  }

  fn _latest_plugin_code(&self) -> Hash {
    self.data().plugin_codes.get(&self.data().plugin_codes_nonce.get_or_default()).unwrap()
  }

  fn _init(&mut self, motherspace_id: AccountId, plugin_code: Hash) {
    self.data().motherspace_id.set(&motherspace_id);
    self._upgrade_plugin_code(plugin_code);
  }

  fn _ensure_motherspace(&self) -> Result<(), LauncherError> {
    ensure!(self.motherspace_id() == Self::env().caller(), LauncherError::UnAuthorized);
    Ok(())
  }
}

pub trait Instantiator {
  /// Internal function which instantiates a shares contract and returns its AccountId
  fn _initiate_new_plugin(&self, _space_id: AccountId, _launcher_id: AccountId, _salt: &[u8]) -> Result<AccountId, LauncherError> {
    Err(LauncherError::Custom(String::from("TODD implement: _initiate_new_plugin")))
  }
}