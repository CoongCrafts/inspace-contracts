// use ink::ToAccountId;
// use ink::env::call::{build_create, ExecutionInput, Selector};
// use ink::prelude::string::String;
// use openbrush::traits::DefaultEnv;
// use openbrush::{
//   modifiers,
//   traits::{
//     AccountId,
//     Storage,
//     Hash
//   },
//   storage::{Mapping}
// };
// use crate::ensure;
//
// type Version = u32;
// type Nonce = u32;
//
// #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq, Clone)]
// #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
// pub enum LauncherError {
//   Custom(String),
//   UnAuthorized,
// }
//
// pub const STORAGE_KEY: u32 = openbrush::storage_unique_key!(Data);
//
// #[derive(Default, Debug)]
// #[openbrush::storage_item]
// pub struct Data {
//   #[lazy]
//   pub motherspace_id: AccountId,
//   #[lazy]
//   pub plugin_codes_nonce: Nonce,
//   pub plugin_codes: Mapping<Version, Hash>,
//
//   #[lazy]
//   pub launches_count: u32,
// }
//
// #[openbrush::trait_definition]
// pub trait PluginLauncher: Storage<Data> {
//   #[ink(message)]
//   fn latest_plugin_code(&self) -> Hash {
//     self._latest_plugin_code()
//   }
//
//   /// For now, we can only upgrade plugin code via motherspace
//   #[ink(message)]
//   fn upgrade_plugin_code(&mut self, new_code_hash: Hash) -> Result<Version, LauncherError> {
//     self.ensure_motherspace()?;
//     Ok(self._upgrade_plugin_code(new_code_hash))
//   }
//
//   #[ink(message)]
//   fn launches_count(&self) -> u32 {
//     self.data().launches_count.get_or_default()
//   }
//
//   #[ink(message)]
//   fn motherspace_id(&self) -> AccountId {
//     self.data().motherspace_id.get().unwrap()
//   }
//
//   fn _upgrade_plugin_code(&mut self, new_plugin_code: Hash) -> Version {
//     let next_plugin_code_version: Version = self.data().plugin_codes_nonce.get_or_default().checked_add(1).expect("Exceeds number ");
//     self.data().plugin_codes.insert(&next_plugin_code_version, &new_plugin_code);
//     self.data().plugin_codes_nonce.set(&next_plugin_code_version);
//
//     next_plugin_code_version
//   }
//
//   fn _latest_plugin_code(&self) -> Hash {
//     self.data().plugin_codes.get(&self.data().plugin_codes_nonce.get_or_default()).unwrap()
//   }
//
//   fn _init(&mut self, motherspace_id: AccountId, plugin_code: Hash) {
//     self.data().motherspace_id.set(&motherspace_id);
//     self._upgrade_plugin_code(plugin_code);
//   }
//
//   fn ensure_motherspace(&self) -> Result<(), LauncherError> {
//     ensure!(self.motherspace_id() == Self::env().caller(), LauncherError::UnAuthorized);
//     Ok(())
//   }
// }