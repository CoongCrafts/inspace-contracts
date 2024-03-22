use ink::env::call::{build_call, ExecutionInput, Selector};
use ink::env::DefaultEnvironment;
use openbrush::{
  modifier_definition,
  modifiers,
  traits::{
    AccountId,
    Storage,
    Hash,
    String
  },
};
pub use crate::traits::plugin_base;

pub type PluginResult<T> = core::result::Result<T, PluginError>;

#[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum PluginError {
  Custom(String),
  UnAuthorized,
  NotActiveMember,
  NotSpaceOwner,
  SetCodeHashFailed,
}

#[derive(Default, Debug)]
#[openbrush::storage_item]
pub struct Data {
  #[lazy]
  pub space_id: AccountId,

  #[lazy]
  pub launcher_id: AccountId,
}

#[openbrush::trait_definition]
pub trait PluginBase: Storage<Data> {
  #[ink(message)]
  fn space_id(&self) -> AccountId {
    self._space_id()
  }

  #[ink(message)]
  fn launcher_id(&self) -> AccountId {
    self._launcher_id()
  }

  #[ink(message)]
  #[modifiers(only_space_owner)]
  fn set_code_hash(&mut self, new_code_hash: Hash) -> PluginResult<()> {
    Self::env()
      .set_code_hash(&new_code_hash)
      .map_err(|_| PluginError::SetCodeHashFailed)
  }

  fn _space_id(&self) -> AccountId {
    self.data().space_id.get().unwrap()
  }

  fn _launcher_id(&self) -> AccountId {
    self.data().launcher_id.get().unwrap()
  }

  fn _ensure_active_member(&self) -> PluginResult<()> {
    let caller = Self::env().caller();

    let is_active_member = build_call::<DefaultEnvironment>()
      .call(self._space_id())
      .gas_limit(0)
      .exec_input(
        ExecutionInput::new(Selector::new(ink::selector_bytes!("is_active_member")))
          .push_arg(caller)
      )
      .returns::<bool>()
      .invoke();

    if is_active_member {
      Ok(())
    } else {
      Err(PluginError::NotActiveMember)
    }
  }

  fn _ensure_space_owner(&self) -> PluginResult<()> {
    let space_owner_id = build_call::<DefaultEnvironment>()
      .call(self._space_id())
      .gas_limit(0)
      .exec_input(
        ExecutionInput::new(Selector::new(ink::selector_bytes!("owner_id")))
      )
      .returns::<AccountId>()
      .invoke();

    let caller = Self::env().caller();

    if space_owner_id == caller {
      Ok(())
    } else {
      Err(PluginError::NotSpaceOwner)
    }
  }

  fn _init(&mut self, space_id: AccountId, launcher_id: AccountId) {
    self.data().space_id.set(&space_id);
    self.data().launcher_id.set(&launcher_id);
  }
}

#[modifier_definition]
pub fn only_space_owner<T, F, R, E>(instance: &mut T, body: F) -> Result<R, E>
  where
    T: Storage<Data>,
    T: PluginBase,
    F: FnOnce(&mut T) -> Result<R, E>,
    E: From<PluginError>,
{
  instance._ensure_space_owner()?;

  body(instance)
}

#[modifier_definition]
pub fn only_active_member<T, F, R, E>(instance: &mut T, body: F) -> Result<R, E>
  where
    T: Storage<Data>,
    T: PluginBase,
    F: FnOnce(&mut T) -> Result<R, E>,
    E: From<PluginError>,
{
  instance._ensure_active_member()?;

  body(instance)
}