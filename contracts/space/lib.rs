#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use space::{SpaceRef};

#[ink::contract]
mod space {
  use ink::storage::{Mapping, Lazy};
  use ink::prelude::string::String;
  use ink::prelude::vec::Vec;
  use helper_macros::*;

  type Result<T> = core::result::Result<T, Error>;

  const SECS_PER_DAY: u64 = 24 * 60 * 60;
  const MAX_PENDING_REQUESTS: u64 = 500;

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum Error {
    Custom(String),
    MemberExisted(AccountId),
    InsufficientPayment,
    CannotRefundPayment(AccountId, RequestId),
  }

  #[derive(Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct SpaceInfo {
    name: String,
    desc: Option<String>,
  }

  #[derive(Clone, Debug, Copy, Default, PartialEq, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub enum RegistrationType {
    #[default]
    PayToJoin,
    RequestToJoin,
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
    registration: RegistrationType,
    pricing: Pricing,
  }

  impl SpaceConfig {
    /// Calculate time to live (ttl) for a membership
    /// None -> Non expiring
    /// Some -> Expiring in seconds from the approved time
    fn ttl(&self) -> Option<u64> {
      match self.pricing {
        Pricing::Subscription { duration, .. } => Some(SECS_PER_DAY.saturating_mul(duration as u64)),
        _ => None,
      }
    }
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

  type RequestId = u32;
  type RequestApproval = (AccountId, bool);

  #[derive(Clone, Debug, Copy, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct MembershipRequest {
    who: AccountId,
    paid: Balance,
    approved: Option<bool>,
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct PendingRequestsPage {
    items: Vec<MembershipRequest>,
    from: u32,
    per_page: u32,
    has_next_page: bool,
    total: u32,
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct ApprovalSubmissionResult {
    // number of approved requests
    approved: u32,
    // number of rejected requests
    rejected: u32,
    // number of not found requests
    not_found: u32,
  }

  #[ink(storage)]
  #[derive(Default)]
  pub struct Space {
    info: Lazy<SpaceInfo>,
    config: Lazy<SpaceConfig>,
    ownable: Lazy<SpaceOwnable>,

    // Membership
    members_nonce: u32,
    // TODO move this to lazy
    members: Mapping<AccountId, MemberInfo>,

    // Membership requests
    requests: Mapping<RequestId, MembershipRequest>,
    registrant_to_request: Mapping<AccountId, RequestId>,
    pending_requests: Lazy<Vec<RequestId>>,
    requests_nonce: Lazy<u32>,
  }

  impl Space {
    #[ink(constructor)]
    pub fn new(motherspace_id: AccountId,
               owner_id: AccountId,
               space_info: SpaceInfo,
               config: Option<SpaceConfig>) -> Result<Self> {
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

      let space_config = match config {
        Some(one) => one,
        None => Self::default_config()
      };

      instance.config.set(&space_config);

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
      self.ensure_owner(Self::env().caller())?;

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

    /// pay to join
    #[ink(message, payable)]
    pub fn pay_to_join(&mut self, who: Option<AccountId>) -> Result<()> {
      let config = self.config();
      ensure!(config.registration == RegistrationType::PayToJoin, Error::Custom(String::from("Space doesn't support pay to join!")));

      let registrant = who.unwrap_or(Self::env().caller());
      ensure!(self.members.get(registrant).is_none(), Error::MemberExisted(registrant));

      let paid_balance: Balance = self.env().transferred_value();

      let valid_payment = match config.pricing {
        Pricing::Free => true,
        Pricing::OneTimePaid { price } => paid_balance >= price,
        Pricing::Subscription { price, .. } => paid_balance >= price
      };

      ensure!(valid_payment, Error::InsufficientPayment);

      self.do_grant_membership(registrant, config.ttl())
    }

    // TODO update member info (name, logo)
    // TODO renew membership

    /// Register for membership
    #[ink(message, payable)]
    pub fn register_membership(&mut self, who: Option<AccountId>) -> Result<()> {
      let config = self.config();
      ensure!(
        config.registration == RegistrationType::RequestToJoin,
        Error::Custom(String::from("Space doesn't support request to join!"))
      );

      let registrant = who.unwrap_or(Self::env().caller());
      ensure!(
        self.members.get(registrant).is_none(),
        Error::MemberExisted(registrant)
      );

      let mut pending_requests = self.pending_requests.get_or_default();

      let maybe_request_id = self.registrant_to_request.get(registrant);
      if let Some(existing_request_id) = maybe_request_id {
        ensure!(
          !pending_requests.contains(&existing_request_id),
          Error::Custom(String::from("The registrant is already having a pending request!"))
        )
      }

      ensure!(
        pending_requests.len() as u64 <= MAX_PENDING_REQUESTS,
        Error::Custom(String::from("Exceeding maximum of pending requests"))
      );

      let next_request_id = self.requests_nonce.get_or_default().checked_add(1).expect("Exceeding number of requests!");

      let paid_balance: Balance = self.env().transferred_value();
      let valid_payment = match config.pricing {
        Pricing::Free => true,
        Pricing::OneTimePaid { price } => paid_balance >= price,
        Pricing::Subscription { price, .. } => paid_balance >= price
      };

      ensure!(valid_payment, Error::InsufficientPayment);

      self.requests_nonce.set(&next_request_id);

      pending_requests.push(next_request_id);
      self.pending_requests.set(&pending_requests);

      self.requests.insert(
        next_request_id,
        &MembershipRequest { who: registrant, paid: paid_balance, approved: None },
      );

      self.registrant_to_request.insert(registrant, &next_request_id);

      Ok(())
    }

    /// get number of pending requests
    #[ink(message)]
    pub fn pending_requests_count(&self) -> u64 {
      self.pending_requests.get_or_default().len() as u64
    }

    /// TODO available to request membership

    /// Get pending request for an account id if has any
    #[ink(message)]
    pub fn pending_request_for(&self, who: Option<AccountId>) -> Option<MembershipRequest> {
      let registrant = who.unwrap_or(Self::env().caller());
      self.get_membership_request(registrant).map(|x| x.1)
    }

    pub fn get_membership_request(&self, who: AccountId) -> Option<(RequestId, MembershipRequest)> {
      let maybe_request_id = self.registrant_to_request.get(who);

      match maybe_request_id {
        None => None,
        Some(request_id) => {
          if self.pending_requests.get_or_default().contains(&request_id) {
            Some((request_id, self.requests.get(request_id).unwrap()))
          } else {
            None
          }
        }
      }
    }

    // Improvements
    // pub fn get_membership_requests(&self, who: Vec<AccountId>) -> Vec<(AccountId, Option<MembershipRequest>)> {
    //   let who_to_request: Vec<(AccountId, Option<RequestId>)> = who.iter().map(|x| (x, self.registrant_to_request.get(x))).collect();
    //   let pending_requests = self.pending_requests.get_or_default();
    //
    //   who_to_request.iter().map(|(who, maybe_request_id)| {
    //     match maybe_request_id {
    //       None => (who, None),
    //       Some(request_id) => {
    //         if pending_requests.contains(&request_id) {
    //           (who, Some(self.requests.get(request_id).unwrap()))
    //         } else {
    //           (who, None)
    //         }
    //       }
    //     }
    //   }).collect::<Vec<(AccountId, Option<MembershipRequest>)>>()
    // }

    /// Get list of pending membership
    #[ink(message)]
    pub fn pending_requests(&self, from: u32, per_page: u32) -> Result<PendingRequestsPage> {
      let requests = self.pending_requests.get_or_default();
      let last_position = from.saturating_add(per_page);
      let total = requests.len() as u32;
      let page: Option<&[RequestId]> = requests.get((from as usize)..(last_position.min(total) as usize));
      let items = match page {
        Some(list) => list.iter().map(|id| self.requests.get(id).unwrap()).collect(),
        None => Vec::new()
      };

      Ok(PendingRequestsPage {
        items,
        from,
        per_page,
        has_next_page: last_position < total,
        total,
      })
    }

    /// Submit request approvals
    #[ink(message)]
    pub fn submit_request_approvals(&mut self, approvals: Vec<RequestApproval>) -> Result<ApprovalSubmissionResult> {
      self.ensure_owner(self.env().caller())?;

      let mut approved_count: u32 = 0;
      let mut rejected_count: u32 = 0;
      let mut not_found_count: u32 = 0;

      let mut submitted_request_ids: Vec<RequestId> = Vec::new();
      for approval in approvals {
        let (who, approved) = approval;
        if let Some((request_id, mut request)) = self.get_membership_request(who) {
          submitted_request_ids.push(request_id);

          if approved {
            // TODO we should return a list of successful, failed items
            self.do_grant_membership(request.who, self.config.get_or_default().ttl())?;
            approved_count = approved_count.saturating_add(1);
          } else if self.env().transfer(request.who, request.paid).is_ok() {
            rejected_count = rejected_count.saturating_add(1);
          } else {
            return Err(Error::CannotRefundPayment(request.who, request_id));
          }

          // update the approval
          request.approved = Some(approved);
          self.requests.insert(request_id, &request);
        } else {
          not_found_count = not_found_count.saturating_add(1);
        }
      }

      // remove submitted request ids out of the pending request list
      let mut pending_requests = self.pending_requests.get_or_default();
      pending_requests.retain(|x| !submitted_request_ids.contains(x));
      self.pending_requests.set(&pending_requests);

      Ok(ApprovalSubmissionResult {
        approved: approved_count,
        rejected: rejected_count,
        not_found: not_found_count,
      })
    }

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

    #[ink(message)]
    pub fn config(&self) -> SpaceConfig {
      self.config.get().unwrap_or(Self::default_config())
    }

    #[ink(message)]
    pub fn update_config(&mut self, config: SpaceConfig) -> Result<()> {
      self.ensure_owner(Self::env().caller())?;
      self.config.set(&config);

      Ok(())
    }

    fn default_config() -> SpaceConfig {
      SpaceConfig {
        registration: RegistrationType::PayToJoin,
        pricing: Pricing::Free,
      }
    }

    fn ensure_owner(&self, who: AccountId) -> Result<()> {
      ensure!(who == self.owner_id(), Error::Custom(String::from("UnAuthorized!")));

      Ok(())
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
