#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use flipper::{FlipperRef};

#[ink::contract]
mod flipper {
  use ink::env::call::{build_call, ExecutionInput, Selector};
  use ink::env::DefaultEnvironment;
  use ink::prelude::{string::String};
  use ink::storage::{Lazy};

  type Result<T> = core::result::Result<T, Error>;

  #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq, Clone)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum Error {
    Custom(String),
    UnAuthorized,
    NotActiveMember,
    NotSpaceOwner,
  }

  #[ink(storage)]
  #[derive(Default)]
  pub struct Flipper {
    space_id: Lazy<AccountId>,
    launcher_id: Lazy<AccountId>,

    value: bool
  }

  impl Flipper {
    #[ink(constructor)]
    pub fn new(space_id: AccountId, launcher_id: AccountId) -> Self {
      let mut one = Flipper::default();
      one.space_id.set(&space_id);
      one.launcher_id.set(&launcher_id);

      one
    }

    /// Flips the current value of the Flipper's boolean.
    /// Only active member can flip
    #[ink(message)]
    pub fn flip(&mut self) -> Result<()> {
      self.ensure_active_member()?;

      self.value = !self.value;

      Ok(())
    }

    /// Returns the current value of the Flipper's boolean.
    #[ink(message)]
    pub fn get(&self) -> bool {
      self.value
    }

    /// Get space id
    #[ink(message)]
    pub fn space_id(&self) -> AccountId {
      self.get_space_id()
    }

    /// Get launcher id
    #[ink(message)]
    pub fn launcher_id(&self) -> AccountId {
      self.get_launcher_id()
    }

    fn get_space_id(&self) -> AccountId {
      self.space_id.get().unwrap()
    }

    fn get_launcher_id(&self) -> AccountId {
      self.launcher_id.get().unwrap()
    }

    fn ensure_active_member(&self) -> Result<()> {
      let caller = Self::env().caller();

      let is_active_member = build_call::<DefaultEnvironment>()
        .call(self.get_space_id())
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
        Err(Error::NotActiveMember)
      }
    }

    fn ensure_space_owner(&self) -> Result<()> {
      let space_owner_id = build_call::<DefaultEnvironment>()
        .call(self.get_space_id())
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
        Err(Error::NotSpaceOwner)
      }
    }

    /// Upgradeable
    #[ink(message)]
    pub fn set_code_hash(&mut self, code_hash: Hash) -> Result<()> {
      self.ensure_space_owner()?;

      ::ink::env::set_code_hash2::<Environment>(&code_hash)
        .map_err(|err| Error::Custom(::ink::prelude::format!("Failed to `set_code_hash` to {:?} due to {:?}", code_hash, err)))?;

      ::ink::env::debug_println!("Switched code hash to {:?}.", code_hash);

      Ok(())
    }

    #[ink(message)]
    pub fn code_hash(&self) -> Hash {
      self.env().code_hash(&self.env().account_id()).unwrap()
    }
  }
}
