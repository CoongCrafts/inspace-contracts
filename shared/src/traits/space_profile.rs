use ink::prelude::string::String;
use openbrush::{
  modifiers,
  traits::{
    Storage,
    AccountId,
    Balance
  },
  contracts::{ownable::*}
};
use crate::ensure;
pub use crate::traits::space_profile;

#[derive(Debug, scale::Decode, scale::Encode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum SpaceError {
  Custom(String),
  OwnableError(OwnableError),
  UnAuthorized,
  MemberExisted(AccountId),
  InsufficientPayment,
  CannotRefundPayment(AccountId, u32), // AccountId, RequestId
  NotActiveMember,
  MemberNotFound,
  PluginNotFound,
}

impl From<OwnableError> for SpaceError {
  fn from(error: OwnableError) -> Self {
    SpaceError::OwnableError(error)
  }
}

#[derive(Clone, Debug, PartialEq, scale::Decode, scale::Encode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
pub enum ImageSource {
  IpfsCid(String),
  Url(String),
}

#[derive(Debug, Default, scale::Decode, scale::Encode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
pub struct SpaceInfo {
  pub name: String,
  pub desc: Option<String>,
  pub logo: Option<ImageSource>,
}

#[derive(Clone, Debug, Copy, Default, PartialEq, scale::Decode, scale::Encode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
pub enum RegistrationType {
  #[default]
  PayToJoin,
  RequestToJoin,
  InviteOnly,
  // ClaimWithNFT,
}

#[derive(Clone, Debug, Copy, Default, scale::Decode, scale::Encode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
pub enum Pricing {
  #[default]
  Free,
  OneTimePaid { price: Balance },
  Subscription { price: Balance, duration: u32 }, // duration is in days
}

#[derive(Debug, Default, scale::Decode, scale::Encode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
pub struct SpaceConfig {
  pub registration: RegistrationType,
  pub pricing: Pricing,
}

const SECS_PER_DAY: u64 = 24 * 60 * 60;

impl SpaceConfig {
  /// Calculate time to live (ttl) for a membership
  /// None -> Non expiring
  /// Some -> Expiring in seconds from the approved time
  pub fn ttl(&self) -> Option<u64> {
    match self.pricing {
      Pricing::Subscription { duration, .. } => Some(SECS_PER_DAY.saturating_mul(duration as u64)),
      _ => None,
    }
  }
}

#[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum SpaaceInfoError {
  Custom(String),
  OwnableError(OwnableError),
  UnAuthorized,
}

impl From<OwnableError> for SpaaceInfoError {
  fn from(error: OwnableError) -> Self {
    SpaaceInfoError::OwnableError(error)
  }
}

#[derive(Default, Debug)]
#[openbrush::storage_item]
pub struct Data {
  #[lazy]
  pub info: SpaceInfo,
  #[lazy]
  pub config: SpaceConfig,
}

#[openbrush::trait_definition]
pub trait SpaceProfile: Storage<Data> + Storage<ownable::Data> {
  /// Get space info
  #[ink(message)]
  fn info(&self) -> SpaceInfo {
    self.data::<Data>().info.get().unwrap()
  }

  #[ink(message)]
  #[modifiers(only_owner)]
  fn update_info(&mut self, info: SpaceInfo) -> Result<(), SpaceError> {
    // TODO validate to limit maximum of chars for each field
    self.data::<Data>().info.set(&info);

    Ok(())
  }

  #[ink(message)]
  fn config(&self) -> SpaceConfig {
    self.data::<Data>().config.get().unwrap_or(Self::_default_config())
  }

  #[ink(message)]
  #[modifiers(only_owner)]
  fn update_config(&mut self, config: SpaceConfig) -> Result<(), SpaceError> {
    self.data::<Data>().config.set(&Self::_normalize_config(Some(config)));

    Ok(())
  }

  fn _default_config() -> SpaceConfig {
    SpaceConfig {
      registration: RegistrationType::PayToJoin,
      pricing: Pricing::Free,
    }
  }

  fn _normalize_config(maybe_config: Option<SpaceConfig>) -> SpaceConfig {
    match maybe_config {
      Some(mut one) => {
        // Invite only mode only accept free pricing
        // We can later allow payment but this is good for now.
        if one.registration == RegistrationType::InviteOnly {
          one.pricing = Pricing::Free;
        }

        one
      }
      None => Self::_default_config()
    }
  }

  fn _init(&mut self, space_info: SpaceInfo, config: Option<SpaceConfig>) -> Result<(), SpaceError> {
    ensure!(space_info.name.len() <= 30, SpaceError::Custom(String::from("Space name is at max 30 chars")));
    ensure!(space_info.name.len() >= 3, SpaceError::Custom(String::from("Space name must be at least 3 chars")));

    if let Some(desc) = space_info.desc.clone() {
      ensure!(desc.len() <= 200, SpaceError::Custom(String::from("Space description is at max 100 chars")));
    }

    self.data::<Data>().info.set(&space_info);
    self.data::<Data>().config.set(&Self::_normalize_config(config));

    Ok(())
  }
}