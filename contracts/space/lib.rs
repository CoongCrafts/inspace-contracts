#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod space {
  use ink::storage::{Mapping, Lazy};
  use ink::prelude::string::String;
  use helper_macros::*;

  type Result<T> = core::result::Result<T, Error>;

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub enum Error {
    Custom(String),
    MemberExisted(AccountId),
  }

  #[derive(Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct SpaceInfo {
    name: String,
    desc: Option<String>,
  }

  #[derive(Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct SpaceOwnable {
    motherspace_id: AccountId,
    owner_id: AccountId,
  }

  #[derive(Debug, Default, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct MemberInfo {
    name: Option<String>,
    /// None -> non expiring, Some -> expiring
    next_renewal_at: Option<Timestamp>,
  }

  #[ink(storage)]
  #[derive(Default)]
  pub struct Space {
    info: Lazy<SpaceInfo>,
    ownable: Lazy<SpaceOwnable>,

    members_nonce: u32,
    members: Mapping<AccountId, MemberInfo>,
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
      instance.ownable.set(&SpaceOwnable { motherspace_id, owner_id });

      instance.do_grant_membership(owner_id, None)?;

      Ok(instance)
    }

    /// Membership methods
    #[ink(message)]
    pub fn members_count(&self) -> u32 {
      self.members_nonce
    }

    #[ink(message)]
    pub fn grant_membership(&mut self, who: AccountId, ttl: Option<u64>) -> Result<()> {
      // TODO add role based access, so admin can also grant memberships
      // TODO grant multiple membership on one go
      ensure!(self.owner_id() == Self::env().caller(), Error::Custom(String::from("Unauthorized!")));

      self.do_grant_membership(who, ttl)
    }

    fn do_grant_membership(&mut self, who: AccountId, ttl: Option<u64>) -> Result<()> {
      ensure!(self.members.get(who).is_none(), Error::MemberExisted(who));

      let next_members_nonce =
        self.members_nonce
          .checked_add(1)
          .expect("Exceeds number of members");

      let current_timestamp = Self::env().block_timestamp();
      let next_renewal_at = ttl.map(|val|
        current_timestamp.checked_add(val).expect("Cannot extend renewal date")
      );

      let new_member = MemberInfo {
        next_renewal_at,
        ..Default::default()
      };

      self.members.insert(who, &new_member);
      self.members_nonce = next_members_nonce;

      Ok(())
    }

    // TODO renew membership
    // TODO invitation
    // TODO register for membership
    // TODO approve/reject membership request
    // TODO pay to join
    // TODO update member info (name, logo)

    #[ink(message)]
    pub fn is_active_member(&self, who: AccountId) -> bool {
      self.check_active_member(&who)
    }

    fn check_active_member(&self, id: &AccountId) -> bool {
      let maybe_member = self.members.get(id);
      match maybe_member {
        Some(info) => {
          match info.next_renewal_at {
            Some(renewal_at) => renewal_at > Self::env().block_timestamp(),
            None => true
          }
        }
        None => false
      }
    }

    /// Get owner id
    #[ink(message)]
    pub fn owner_id(&self) -> AccountId {
      self.ownable.get().unwrap().owner_id
    }

    /// Get motherspace id
    #[ink(message)]
    pub fn motherspace_id(&self) -> AccountId {
      self.ownable.get().unwrap().motherspace_id
    }

    /// Get space info
    #[ink(message)]
    pub fn info(&self) -> SpaceInfo {
      self.info.get().unwrap()
    }
  }

  use traits::Upgradeable;

  impl Upgradeable for Space {
    #[ink(message)]
    fn set_code_hash(&mut self, code_hash: Hash) {
      assert_eq!(self.owner_id(), Self::env().caller(), "UnAuthorized");
      ink::env::set_code_hash2::<Environment>(&code_hash).unwrap_or_else(|err| {
        panic!(
          "Failed to `set_code_hash` to {:?} due to {:?}",
          code_hash, err
        )
      });
      ink::env::debug_println!("Switched code hash to {:?}.", code_hash);
    }

    #[ink(message)]
    fn code_hash(&self) -> Hash {
      self.env().code_hash(&self.env().account_id()).unwrap()
    }
  }
}
