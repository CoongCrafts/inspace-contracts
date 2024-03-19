#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[openbrush::implementation(Ownable, Upgradeable)]
#[openbrush::contract]
mod motherspace {
  use openbrush::{modifiers, traits::{Storage}};
  use ink::env::{
    DefaultEnvironment,
    call::{build_call, build_create, ExecutionInput, Selector},
  };
  use ink::storage::{Lazy, Mapping};
  use ink::prelude::{format, vec::Vec, string::String};
  use ink::ToAccountId;
  use shared::ensure;
  use shared::traits::codehash::*;
  use space::SpaceRef;

  type MotherSpaceResult<T> = core::result::Result<T, MotherSpaceError>;

  type Nonce = u32;
  // index
  type Version = u32;
  type SpaceId = AccountId;
  type PluginIndex = u32;
  type PluginId = [u8; 4];

  #[derive(Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum MotherSpaceError {
    Custom(String),
    OwnableError(OwnableError),
    UnAuthorized,
    SpaceNotFound,
    PluginNotFound,
    PluginLaunchFailed,
    PluginIdExisted,
  }

  impl From<OwnableError> for MotherSpaceError {
    fn from(error: OwnableError) -> Self {
      MotherSpaceError::OwnableError(error)
    }
  }

  #[derive(Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum ImageSource {
    IpfsCid(String),
    Url(String),
  }

  #[derive(Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct SpaceInfo {
    name: String,
    desc: Option<String>,
    logo: Option<ImageSource>,
  }

  #[derive(Clone, Debug, Copy, Default, PartialEq, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum RegistrationType {
    #[default]
    PayToJoin,
    RequestToJoin,
    InviteOnly,
    // ClaimWithNFT,
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

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct Pagination<Item> {
    items: Vec<Item>,
    from: u32,
    per_page: u32,
    has_next_page: bool,
    total: u32,
  }

  type SpacesPage = Pagination<SpaceId>;

  #[ink(storage)]
  #[derive(Default, Storage)]
  pub struct MotherSpace {
    space_codes: Mapping<Version, Hash>,
    space_codes_nonce: Lazy<Nonce>,

    members_to_spaces: Mapping<AccountId, Vec<SpaceId>>,

    deployed_spaces: Mapping<SpaceId, AccountId>,
    index_to_space: Mapping<u32, SpaceId>,
    spaces_count: Lazy<u32>,

    ids_to_plugin_launchers: Mapping<PluginId, AccountId>,
    plugin_launchers: Mapping<PluginIndex, PluginId>,
    plugins_nonce: Lazy<Nonce>,

    #[storage_field]
    ownable: ownable::Data,
  }

  impl CodeHash for MotherSpace {}

  impl MotherSpace {
    #[ink(constructor)]
    pub fn new(space_code: Hash, owner_id: AccountId) -> Self {
      let mut one = MotherSpace::default();
      ownable::Internal::_init_with_owner(&mut one, owner_id);
      one.upgrade_space_code_impl(space_code);

      one
    }

    #[ink(message)]
    #[modifiers(only_owner)]
    pub fn upgrade_space_code(&mut self, new_space_code: Hash) -> MotherSpaceResult<()> {
      self.upgrade_space_code_impl(new_space_code);

      Ok(())
    }

    #[ink(message)]
    pub fn latest_space_code(&self) -> Hash {
      self.latest_space_code_impl()
    }

    #[ink(message)]
    pub fn deploy_new_space(&mut self, info: SpaceInfo, config: Option<SpaceConfig>,
                            owner: Option<AccountId>, plugins: Option<Vec<PluginId>>) -> MotherSpaceResult<(SpaceId, Vec<(PluginId, AccountId)>)> {
      let new_spaces_count = self.spaces_count.get_or_default();

      let motherspace_id = Self::env().account_id();
      let owner_id = owner.unwrap_or(Self::env().caller());

      let new_space: SpaceRef = build_create::<SpaceRef>()
        .code_hash(self.latest_space_code_impl())
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

      self.deployed_spaces.insert(new_space_id, &owner_id);
      self.index_to_space.insert(new_spaces_count, &new_space_id);

      let next_spaces_count = new_spaces_count.saturating_add(1);
      self.spaces_count.set(&next_spaces_count);

      self.add_space_member_impl(new_space_id, owner_id);

      // TODO should emit errors if plugins fail to deploy
      let deployed_plugins = match plugins {
        Some(plugin_ids) => self.install_plugins_impl(new_space_id, plugin_ids).unwrap(),
        None => Vec::new()
      };

      Ok((new_space_id, deployed_plugins))
    }

    #[ink(message)]
    pub fn list_spaces(&self, from: u32, per_page: u32) -> SpacesPage {
      let last_position = from.saturating_add(per_page);
      let per_page = per_page.min(50); // limit per page at max 50 items
      let current_spaces_count = self.spaces_count.get_or_default();

      let mut space_records = Vec::new();
      for index in (from as usize)..(last_position.min(current_spaces_count) as usize) {
        let bounded_index = index as u32;
        if let Some(space_id) = self.index_to_space.get(bounded_index) {
          space_records.push(space_id)
        }
      }

      SpacesPage {
        items: space_records,
        from,
        per_page,
        has_next_page: last_position < current_spaces_count,
        total: current_spaces_count,
      }
    }

    #[ink(message)]
    pub fn member_spaces(&self, who: Option<AccountId>) -> Vec<SpaceId> {
      let who = who.unwrap_or(self.env().caller());
      self.members_to_spaces.get(who).unwrap_or_default()
    }

    #[ink(message)]
    pub fn add_space_member(&mut self, who: AccountId) -> MotherSpaceResult<()> {
      let space_id = self.env().caller();
      ensure!(self.is_deployed_space_impl(space_id), MotherSpaceError::Custom(String::from("Only deployed spaces can call this!")));

      self.add_space_member_impl(space_id, who);

      Ok(())
    }

    #[ink(message)]
    pub fn remove_space_member(&mut self, who: AccountId) -> MotherSpaceResult<()> {
      let space_id = self.env().caller();
      ensure!(self.is_deployed_space_impl(space_id), MotherSpaceError::Custom(String::from("Only deployed spaces can call this!")));

      self.remove_space_member_impl(space_id, who);

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
    pub fn plugins_count(&self) -> u32 {
      self.plugins_nonce.get_or_default()
    }

    #[ink(message)]
    #[modifiers(only_owner)]
    pub fn register_plugin_launcher(&mut self, plugin_id: PluginId, launcher_address: AccountId) -> MotherSpaceResult<PluginIndex> {
      // For now only owner can register plugin launcher
      // Later we can add a mechanism for anyone can submit a plugin application for approval
      ensure!(!self.ids_to_plugin_launchers.contains(plugin_id), MotherSpaceError::PluginIdExisted);

      let new_plugin_id = self.plugins_nonce.get_or_default();
      self.plugin_launchers.insert(new_plugin_id, &plugin_id);
      self.ids_to_plugin_launchers.insert(plugin_id, &launcher_address);
      self.plugins_nonce.set(&new_plugin_id.checked_add(1).expect("Exceeds number of plugins"));

      Ok(new_plugin_id)
    }

    /// Update plugin launcher address or remove it
    // #[ink(message)]
    // [modifiers(only_owner)]
    // pub fn update_plugin_launcher(&mut self, plugin_id: PluginId, launcher_address: Option<AccountId>) -> MotherSpaceResult<()> {
    //   let ZERO_ACCOUNT: AccountId = [0; 32].into();
    //   ensure!(self.ids_to_plugin_launchers.contains(plugin_id), MotherSpaceError::PluginNotFound);
    //   let new_address = launcher_address.unwrap_or(ZERO_ACCOUNT);
    //   self.ids_to_plugin_launchers.insert(plugin_id, &new_address);
    //
    //   Ok(())
    // }

    /// For the sake of simplicity, get full list of plugin launcher
    /// We'll need to add pagination later
    #[ink(message)]
    pub fn plugin_launchers(&self) -> Vec<(PluginId, AccountId)> {
      let mut launchers: Vec<(PluginId, AccountId)> = Vec::new();

      for idx in 0..(self.plugins_count()) {
        if let Some(plugin_id) = self.plugin_launchers.get(idx) {
          if let Some(launcher_address) = self.ids_to_plugin_launchers.get(plugin_id) {
            launchers.push((plugin_id, launcher_address));
          }
        }
      }

      launchers
    }

    #[ink(message)]
    pub fn latest_plugin_code(&self, plugin_id: PluginId) -> MotherSpaceResult<Hash> {
      let launcher = self.ids_to_plugin_launchers.get(plugin_id).ok_or(MotherSpaceError::PluginNotFound)?;

      let result = build_call::<DefaultEnvironment>()
        .call(launcher)
        .gas_limit(0)
        .exec_input(
          ExecutionInput::new(Selector::new(ink::selector_bytes!("latest_plugin_code")))
        )
        .returns::<Hash>()
        .invoke();

      Ok(result)
    }

    #[ink(message)]
    #[modifiers(only_owner)]
    pub fn upgrade_plugin_code(&mut self, plugin_id: PluginId, new_code_hash: Hash) -> MotherSpaceResult<Version> {
      let launcher = self.ids_to_plugin_launchers.get(plugin_id).ok_or(MotherSpaceError::PluginNotFound)?;

      let new_version = build_call::<DefaultEnvironment>()
        .call(launcher)
        .gas_limit(0)
        .exec_input(
          ExecutionInput::new(Selector::new(ink::selector_bytes!("upgrade_plugin_code")))
            .push_arg(new_code_hash)
        )
        .returns::<Version>()
        .invoke();

      Ok(new_version)
    }


    /// Install plugins
    #[ink(message)]
    pub fn install_plugins(&mut self, space_id: SpaceId, plugins: Vec<PluginId>) -> MotherSpaceResult<Vec<(PluginId, AccountId)>> {
      if !self.is_deployed_space(space_id) {
        return Err(MotherSpaceError::SpaceNotFound);
      }

      // Ensure space owner
      let space_owner_id = build_call::<DefaultEnvironment>()
        .call(space_id)
        .gas_limit(0)
        .exec_input(
          ExecutionInput::new(Selector::new(ink::selector_bytes!("owner_id")))
        )
        .returns::<AccountId>()
        .invoke();

      ensure!(space_owner_id == self.env().caller(), MotherSpaceError::UnAuthorized);

      self.install_plugins_impl(space_id, plugins)
    }

    fn install_plugins_impl(&mut self, space_id: SpaceId, plugins: Vec<PluginId>) -> MotherSpaceResult<Vec<(PluginId, AccountId)>> {
      let mut deployed_plugins: Vec<(PluginId, AccountId)> = Vec::new();
      for plugin_id in plugins {
        let opt_launcher = self.ids_to_plugin_launchers.get(plugin_id);
        if let Some(launcher_address) = opt_launcher {
          let plugin_address_rs = build_call::<DefaultEnvironment>()
            .call(launcher_address)
            .gas_limit(0)
            .exec_input(
              ExecutionInput::new(Selector::new(ink::selector_bytes!("launch")))
                .push_arg(space_id)
            )
            .returns::<MotherSpaceResult<AccountId>>()
            .invoke();

          if let Ok(plugin_address) = plugin_address_rs {
            deployed_plugins.push((plugin_id, plugin_address));
          } else {
            return Err(MotherSpaceError::PluginLaunchFailed);
          }
        }
      }

      if deployed_plugins.is_empty() {
        return Ok(deployed_plugins);
      }

      ::ink::env::debug_println!("Deployed plugins {:?}", deployed_plugins);

      let result = build_call::<DefaultEnvironment>()
        .call(space_id)
        .gas_limit(0)
        .exec_input(
          ExecutionInput::new(Selector::new(ink::selector_bytes!("attach_plugins")))
            .push_arg(&deployed_plugins)
        )
        .returns::<MotherSpaceResult<()>>()
        .invoke();

      if result.is_ok() {
        Ok(deployed_plugins)
      } else {
        Err(MotherSpaceError::Custom(format!("Attach plugin failed, error: {:?}", result.unwrap_err())))
      }
    }

    fn latest_space_code_impl(&self) -> Hash {
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

    fn remove_space_member_impl(&mut self, space_id: SpaceId, member_id: AccountId) {
      let owner_spaces = self.members_to_spaces.get(member_id).unwrap_or_default();
      if owner_spaces.contains(&space_id) {
        let new_spaces: Vec<AccountId> = owner_spaces.into_iter().filter(|&x| x != space_id).collect();
        self.members_to_spaces.insert(member_id, &new_spaces);
      }
    }

    fn upgrade_space_code_impl(&mut self, new_space_code: Hash) {
      let next_space_code_version: Version = self.space_codes_nonce.get_or_default().checked_add(1).expect("Cannot upgrade space code!");
      self.space_codes.insert(next_space_code_version, &new_space_code);
      self.space_codes_nonce.set(&next_space_code_version);
    }
  }
}
