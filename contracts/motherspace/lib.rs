#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod motherspace {
  use ink::env::call::{build_create, ExecutionInput, Selector};
  use traits::{Upgradeable};
  use ink::storage::{Lazy, Mapping};
  use ink::prelude::string::String;
  use ink::prelude::vec::Vec;
  use ink::ToAccountId;
  use helper_macros::*;
  use space::SpaceRef;

  type Result<T> = core::result::Result<T, Error>;
  type Version = u32;
  type CodeHash = Hash;
  type SpaceId = AccountId;

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum Error {
    Custom(String),
  }

  #[derive(Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct SpaceInfo {
    name: String,
    desc: Option<String>,
  }

  #[derive(Clone, Debug, Copy, Default, PartialEq, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum RegistrationType {
    #[default]
    PayToJoin,
    RequestToJoin,
  }

  #[derive(Clone, Debug, Copy, Default, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum Pricing {
    #[default]
    Free,
    OneTimePaid { price: Balance },
    Subscription { price: Balance, duration: u32 }, // duration is in days
  }

  #[derive(Debug, Default, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct SpaceConfig {
    registration: RegistrationType,
    pricing: Pricing,
  }

  #[ink(storage)]
  #[derive(Default)]
  pub struct MotherSpace {
    space_codes: Mapping<Version, CodeHash>,
    space_codes_nonce: Lazy<Version>,

    members_to_spaces: Mapping<AccountId, Vec<SpaceId>>,

    deployed_spaces: Mapping<SpaceId, AccountId>,
    spaces_count: Lazy<u32>,

    owner_id: Lazy<AccountId>,
  }

  impl MotherSpace {
    /// Constructor that initializes the `bool` value to the given `init_value`.
    #[ink(constructor)]
    pub fn new(space_code: Hash, owner_id: AccountId) -> Self {
      let mut instance = MotherSpace::default();
      instance.owner_id.set(&owner_id);

      let initial_space_version: Version = 1;
      instance.space_codes.insert(initial_space_version, &space_code);
      instance.space_codes_nonce.set(&initial_space_version);

      instance
    }

    #[ink(message)]
    pub fn deploy_new_space(&mut self, info: SpaceInfo, config: Option<SpaceConfig>) -> Result<SpaceId> {
      let new_spaces_count = self.spaces_count().saturating_add(1);

      let motherspace_id = Self::env().account_id();
      let owner_id = Self::env().caller();

      let new_space: SpaceRef = build_create::<SpaceRef>()
        .code_hash(self.latest_space_code())
        .gas_limit(0)
        .endowment(0)
        .exec_input(
          ExecutionInput::new(Selector::new(ink::selector_bytes!("new")))
            .push_arg(motherspace_id)
            .push_arg(owner_id)
            .push_arg(&info)
            .push_arg(&config)
        )
        .salt_bytes(new_spaces_count.to_le_bytes())
        .returns::<SpaceRef>()
        .instantiate();

      let new_space_id = new_space.to_account_id();

      self.spaces_count.set(&new_spaces_count);
      self.deployed_spaces.insert(new_space_id, &owner_id);

      self.add_space_member_impl(new_space_id, owner_id);

      Ok(new_space_id)
    }

    #[ink(message)]
    pub fn member_spaces(&self, who: Option<AccountId>) -> Vec<SpaceId> {
      let who = who.unwrap_or(self.env().caller());
      self.members_to_spaces.get(who).unwrap_or_default()
    }

    #[ink(message)]
    pub fn add_space_member(&mut self, who: AccountId) -> Result<()> {
      let space_id = self.env().caller();
      ensure!(self.is_deployed_space_impl(space_id), Error::Custom(String::from("Only deployed spaces can call this!")));

      self.add_space_member_impl(space_id, who);

      Ok(())
    }

    #[ink(message)]
    pub fn spaces_count(&self) -> u32 {
      self.spaces_count.get_or_default()
    }

    #[ink(message)]
    pub fn is_deployed_space(&self, space_id: SpaceId) -> bool {
      self.is_deployed_space_impl(space_id)
    }

    #[ink(message)]
    pub fn owner_id(&self) -> AccountId {
      self.owner_id.get().unwrap()
    }

    fn latest_space_code(&self) -> CodeHash {
      self.space_codes.get(self.space_codes_nonce.get_or_default()).unwrap()
    }

    fn is_deployed_space_impl(&self, space_id: SpaceId) -> bool {
      self.deployed_spaces.contains(space_id)
    }

    fn add_space_member_impl(&mut self, space_id: SpaceId, member_id: AccountId) {
      let mut owner_spaces = self.members_to_spaces.get(member_id).unwrap_or_default();
      if !owner_spaces.contains(&space_id) {
        owner_spaces.push(space_id);
        self.members_to_spaces.insert(member_id, &owner_spaces);
      }
    }
  }

  impl Upgradeable for MotherSpace {
    #[ink(message)]
    fn set_code_hash(&mut self, code_hash: Hash) {
      assert_eq!(self.owner_id(), Self::env().caller(), "UnAuthorized");
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
