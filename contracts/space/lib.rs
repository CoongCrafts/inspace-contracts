#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use space::{SpaceRef};

#[openbrush::implementation(Ownable, Upgradeable)]
#[openbrush::contract]
mod space {
  use ink::env::call::{build_call, ExecutionInput, Selector};
  use ink::env::{DefaultEnvironment};
  use ink::storage::{Mapping, Lazy};
  use ink::prelude::string::String;
  use ink::prelude::vec::Vec;
  use openbrush::{modifiers, traits::Storage};
  use shared::ensure;
  use shared::traits::codehash::*;
  use shared::traits::space_profile::*;

  type SpaceResult<T> = core::result::Result<T, SpaceError>;

  const MAX_PENDING_REQUESTS: u64 = 500;

  #[derive(Clone, Debug, Default, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct MemberInfo {
    name: Option<String>,
    /// None -> non expiring, Some(>0) -> expiring, Some(0) -> member already left
    next_renewal_at: Option<Timestamp>,
    joined_at: Timestamp,
  }

  type RequestId = u32;
  type RequestApproval = (AccountId, bool);

  type PluginId = [u8; 4];

  #[derive(Clone, Debug, Copy, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct MembershipRequest {
    who: AccountId,
    paid: Balance,
    requested_at: Timestamp,
    approved: Option<bool>,
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

  type PendingRequestsPage = Pagination<MembershipRequest>;

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct MemberRecord {
    index: u32,
    account_id: AccountId,
    info: MemberInfo,
  }

  type MembersPage = Pagination<MemberRecord>;

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

  #[derive(Clone, Debug, PartialEq, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum MemberStatus {
    None,
    Active, // nextRenewalAt >= now
    Inactive, // 0 < nextRenewalAt < now
    Left, // nextRenewalAt == 0, was a member before but already left
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct PluginInfo {
    id: PluginId,
    address: AccountId,
    disabled: bool,
    code_hash: Hash
  }

  #[ink(storage)]
  #[derive(Default, Storage)]
  pub struct Space {
    #[storage_field]
    profile: space_profile::Data,

    // Membership
    members_nonce: Lazy<u32>,
    members: Mapping<AccountId, MemberInfo>,
    index_to_member: Mapping<u32, AccountId>,

    // Membership requests
    requests: Mapping<RequestId, MembershipRequest>,
    registrant_to_request: Mapping<AccountId, RequestId>,
    pending_requests: Lazy<Vec<RequestId>>,
    requests_nonce: Lazy<u32>,

    // plugins
    plugins: Mapping<PluginId, AccountId>,
    disabled_plugin_ids: Lazy<Vec<PluginId>>,
    plugin_ids: Lazy<Vec<PluginId>>,

    #[storage_field]
    ownable: ownable::Data,
    motherspace_id: Lazy<AccountId>,
  }

  impl CodeHash for Space {}
  impl SpaceProfile for Space {}

  impl Space {
    #[ink(constructor)]
    pub fn new(motherspace_id: AccountId,
               owner_id: AccountId,
               space_info: SpaceInfo,
               config: Option<SpaceConfig>) -> SpaceResult<Self> {
      ensure!(motherspace_id == Self::env().caller(), SpaceError::Custom(String::from("Only MotherSpace can deploy spaces!")));

      let mut instance = Space::default();

      space_profile::SpaceProfile::_init(&mut instance, space_info, config)?;
      ownable::Internal::_init_with_owner(&mut instance, owner_id);

      instance.motherspace_id.set(&motherspace_id);
      instance.do_grant_membership(owner_id, None, false)?;

      Ok(instance)
    }

    /// Attach plugins to space, motherspace call this when install plugins for spaces
    #[ink(message)]
    pub fn attach_plugins(&mut self, plugins: Vec<(PluginId, AccountId)>) -> SpaceResult<()> {
      ensure!(self.motherspace_id() == Self::env().caller(), SpaceError::Custom(String::from("Only MotherSpace can attach plugins!")));

      if plugins.iter().any(|&p| self.plugins.contains(p.0)) {
        return Err(SpaceError::Custom(String::from("Cannot attach a plugin more than one.")));
      }

      let mut plugin_ids = self.plugin_ids.get_or_default();
      for (id, address) in plugins {
        self.plugins.insert(id, &address);
        plugin_ids.push(id);
      }

      self.plugin_ids.set(&plugin_ids);

      Ok(())
    }

    #[ink(message)]
    pub fn plugins(&self) -> Vec<PluginInfo> {
      self.plugin_ids.get_or_default()
        .iter()
        .map(|&id| PluginInfo {
          id,
          address: self.plugins.get(id).unwrap(),
          disabled: self.disabled_plugin_ids.get_or_default().contains(&id),
          code_hash: self._plugin_code_hash(id).unwrap(),
        })
        .collect()
    }

    #[ink(message)]
    #[modifiers(only_owner)]
    pub fn enable_plugin(&mut self, plugin_id: PluginId) -> SpaceResult<()> {
      ensure!(self.plugin_ids.get_or_default().contains(&plugin_id), SpaceError::PluginNotFound);

      let mut disabled_ids = self.disabled_plugin_ids.get_or_default();
      disabled_ids.retain(|&x| x != plugin_id);
      self.disabled_plugin_ids.set(&disabled_ids);

      Ok(())
    }

    #[ink(message)]
    #[modifiers(only_owner)]
    pub fn disable_plugin(&mut self, plugin_id: PluginId) -> SpaceResult<()> {
      let plugin_ids = self.plugin_ids.get_or_default();
      ensure!(plugin_ids.contains(&plugin_id), SpaceError::PluginNotFound);

      let mut disabled_ids = self.disabled_plugin_ids.get_or_default();
      if !disabled_ids.contains(&plugin_id) {
        disabled_ids.push(plugin_id);
        self.disabled_plugin_ids.set(&disabled_ids);
      }

      Ok(())
    }

    #[ink(message)]
    pub fn plugin_code_hash(&self, plugin_id: PluginId) -> SpaceResult<Hash> {
      self._plugin_code_hash(plugin_id)
    }

    fn _plugin_code_hash(&self, plugin_id: PluginId) -> SpaceResult<Hash> {
      let plugin_ids = self.plugin_ids.get_or_default();
      ensure!(plugin_ids.contains(&plugin_id), SpaceError::PluginNotFound);
      let plugin_address = self.plugins.get(plugin_id).ok_or(SpaceError::PluginNotFound)?;

      let code_hash = build_call::<DefaultEnvironment>()
        .call(plugin_address)
        .gas_limit(0)
        .exec_input(
          ExecutionInput::new(Selector::new(ink::selector_bytes!("CodeHash::code_hash")))
        )
        .returns::<Hash>()
        .invoke();

      Ok(code_hash)
    }

    /// Membership methods
    #[ink(message)]
    pub fn members_count(&self) -> u32 {
      self.members_nonce.get_or_default()
    }

    #[ink(message)]
    pub fn list_members(&self, from: u32, per_page: u32) -> MembersPage {
      let last_position = from.saturating_add(per_page);
      let per_page = per_page.min(50); // limit per page at max 50 items
      let current_member_nonce = self.members_nonce.get_or_default();

      let mut member_records = Vec::new();
      for index in (from as usize)..(last_position.min(current_member_nonce) as usize) {
        let bounded_index = index as u32;

        if let Some(account_id) = self.index_to_member.get(bounded_index) {
          if let Some(info) = self.members.get(account_id) {
            member_records.push(MemberRecord { index: bounded_index, account_id, info })
          }
        }
      }

      MembersPage {
        items: member_records,
        from,
        per_page,
        has_next_page: last_position < current_member_nonce,
        total: current_member_nonce,
      }
    }

    #[ink(message)]
    #[modifiers(only_owner)]
    pub fn grant_membership(&mut self, who: AccountId, ttl: Option<u64>) -> SpaceResult<()> {
      // TODO add role based access, so admin can also grant memberships
      // TODO grant multiple membership on one go

      self.do_grant_membership(who, ttl, true)
    }

    fn do_grant_membership(&mut self, who: AccountId, ttl: Option<u64>, register_space_member: bool) -> SpaceResult<()> {
      let member_status = self.member_status(who);
      ensure!(member_status != MemberStatus::Active, SpaceError::MemberExisted(who));

      let current_timestamp = Self::env().block_timestamp();
      let next_renewal_at = ttl.map(|val|
        current_timestamp.checked_add(val).expect("Cannot extend renewal date")
      );

      if member_status == MemberStatus::None {
        let new_member = MemberInfo {
          next_renewal_at,
          joined_at: current_timestamp,
          ..Default::default()
        };

        let current_members_nonce = self.members_nonce.get_or_default();
        let next_members_nonce =
          current_members_nonce
            .checked_add(1)
            .expect("Exceeds number of members");

        self.members.insert(who, &new_member);
        self.index_to_member.insert(current_members_nonce, &who);
        self.members_nonce.set(&next_members_nonce);
      } else {
        let mut member_info = self.members.get(who).unwrap();
        member_info.next_renewal_at = next_renewal_at;

        self.members.insert(who, &member_info);
      }

      // Register space member in mother space
      if register_space_member {
        let _ = build_call::<DefaultEnvironment>()
          .call(self.motherspace_id())
          .gas_limit(0)
          .exec_input(
            ExecutionInput::new(Selector::new(ink::selector_bytes!("add_space_member")))
              .push_arg(who)
          )
          .returns::<SpaceResult<()>>()
          .invoke();
      }

      Ok(())
    }

    /// pay to join
    #[ink(message, payable)]
    pub fn pay_to_join(&mut self, who: Option<AccountId>) -> SpaceResult<()> {
      let config = self.config();
      ensure!(config.registration == RegistrationType::PayToJoin, SpaceError::Custom(String::from("Space doesn't support pay to join!")));

      let registrant = who.unwrap_or(self.env().caller());
      ensure!(!self.is_member(Some(registrant)), SpaceError::MemberExisted(registrant));

      let paid_balance: Balance = self.env().transferred_value();

      let valid_payment = match config.pricing {
        Pricing::Free => true,
        Pricing::OneTimePaid { price } => paid_balance >= price,
        Pricing::Subscription { price, .. } => paid_balance >= price
      };

      ensure!(valid_payment, SpaceError::InsufficientPayment);

      self.do_grant_membership(registrant, config.ttl(), true)
    }

    // TODO renew membership

    /// Register for membership
    #[ink(message, payable)]
    pub fn register_membership(&mut self, who: Option<AccountId>) -> SpaceResult<()> {
      let config = self.config();
      ensure!(
        config.registration == RegistrationType::RequestToJoin,
        SpaceError::Custom(String::from("Space doesn't support request to join!"))
      );

      let registrant = who.unwrap_or(Self::env().caller());
      ensure!(!self.is_member(Some(registrant)), SpaceError::MemberExisted(registrant));

      let mut pending_requests = self.pending_requests.get_or_default();

      let maybe_request_id = self.registrant_to_request.get(registrant);
      if let Some(existing_request_id) = maybe_request_id {
        ensure!(
          !pending_requests.contains(&existing_request_id),
          SpaceError::Custom(String::from("The registrant is already having a pending request!"))
        );
      }

      ensure!(
        pending_requests.len() as u64 <= MAX_PENDING_REQUESTS,
        SpaceError::Custom(String::from("Exceeding maximum of pending requests"))
      );

      let next_request_id = self.requests_nonce.get_or_default().checked_add(1).expect("Exceeding number of requests!");

      let paid_balance: Balance = self.env().transferred_value();
      let valid_payment = match config.pricing {
        Pricing::Free => true,
        Pricing::OneTimePaid { price } => paid_balance >= price,
        Pricing::Subscription { price, .. } => paid_balance >= price
      };

      ensure!(valid_payment, SpaceError::InsufficientPayment);

      self.requests_nonce.set(&next_request_id);

      pending_requests.push(next_request_id);
      self.pending_requests.set(&pending_requests);

      self.requests.insert(
        next_request_id,
        &MembershipRequest {
          who: registrant,
          paid: paid_balance,
          requested_at: self.env().block_timestamp(),
          approved: None,
        },
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

    #[ink(message)]
    pub fn cancel_request(&mut self) -> SpaceResult<()> {
      let caller = self.env().caller();

      let maybe_request = self.get_membership_request(caller);
      ensure!(maybe_request.is_some(), SpaceError::Custom(String::from("Request Not Found")));

      let (request_id, request) = maybe_request.unwrap();

      // Refund the payment
      if self.env().transfer(caller, request.paid).is_err() {
        return Err(SpaceError::CannotRefundPayment(request.who, request_id));
      }

      let mut pending_requests = self.pending_requests.get_or_default();
      pending_requests.retain(|&x| x != request_id);
      self.pending_requests.set(&pending_requests);

      Ok(())
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
    pub fn pending_requests(&self, from: u32, per_page: u32) -> PendingRequestsPage {
      let per_page = per_page.min(50); // limit per page at max 50 items
      let requests = self.pending_requests.get_or_default();
      let last_position = from.saturating_add(per_page);
      let total = requests.len() as u32;
      let page: Option<&[RequestId]> = requests.get((from as usize)..(last_position.min(total) as usize));
      let items = match page {
        Some(list) => list.iter().map(|id| self.requests.get(id).unwrap()).collect(),
        None => Vec::new()
      };

      PendingRequestsPage {
        items,
        from,
        per_page,
        has_next_page: last_position < total,
        total,
      }
    }

    /// Submit request approvals
    #[ink(message)]
    #[modifiers(only_owner)]
    pub fn submit_request_approvals(&mut self, approvals: Vec<RequestApproval>) -> SpaceResult<ApprovalSubmissionResult> {
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
            self.do_grant_membership(request.who, self.profile.config.get_or_default().ttl(), true)?;
            approved_count = approved_count.saturating_add(1);
          } else if self.env().transfer(request.who, request.paid).is_ok() {
            rejected_count = rejected_count.saturating_add(1);
          } else {
            return Err(SpaceError::CannotRefundPayment(request.who, request_id));
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

    /// Check member status
    #[ink(message)]
    pub fn member_status(&self, who: AccountId) -> MemberStatus {
      let maybe_member = self.members.get(who);
      match maybe_member {
        Some(info) => {
          match info.next_renewal_at {
            Some(renewal_at) => {
              if renewal_at > Self::env().block_timestamp() {
                MemberStatus::Active
              } else if renewal_at > 0 {
                MemberStatus::Inactive
              } else {
                MemberStatus::Left
              }
            }
            None => MemberStatus::Active
          }
        }
        None => MemberStatus::None
      }
    }

    /// Active member to leave space
    /// For now, only the member himself can call this
    /// Later we can consider allow owner to do this
    /// or a voting mechanism to force a member to leave
    #[ink(message)]
    pub fn leave(&mut self) -> SpaceResult<()> {
      let who = self.env().caller();

      ensure!(who != Ownable::owner(self).unwrap(), SpaceError::Custom(String::from("Owner cannot leave the space")));

      let member_status = self.member_status(who);
      ensure!(member_status == MemberStatus::Active, SpaceError::NotActiveMember);
      let mut member_info = self.members.get(who).unwrap();
      member_info.next_renewal_at = Some(0);

      self.members.insert(who, &member_info);

      // Remove space member tracking
      let _ = build_call::<DefaultEnvironment>()
        .call(self.motherspace_id())
        .gas_limit(0)
        .exec_input(
          ExecutionInput::new(Selector::new(ink::selector_bytes!("remove_space_member")))
            .push_arg(who)
        )
        .returns::<SpaceResult<()>>()
        .invoke();

      Ok(())
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

    /// Get motherspace id
    #[ink(message)]
    pub fn motherspace_id(&self) -> AccountId {
      self.motherspace_id.get().unwrap()
    }

    /// Member info
    #[ink(message)]
    pub fn member_info(&self, who: AccountId) -> Option<MemberInfo> {
      self.members.get(who)
    }

    #[ink(message)]
    pub fn update_member_info(&mut self, name: Option<String>) -> SpaceResult<()> {
      let caller = self.env().caller();

      ensure!(self.check_active_member(&caller), SpaceError::NotActiveMember);
      if let Some(new_name) = &name {
        ensure!(new_name.len() >= 3, SpaceError::Custom(String::from("Display name must be a least 3 characters")));
        ensure!(new_name.len() <= 30, SpaceError::Custom(String::from("Display name must be at most 30 characters")));
      }

      let updated_member_info = self
        .members
        .get(caller)
        .map(|member_info| MemberInfo {
          name,
          ..member_info
        })
        .unwrap();

      self.members.insert(caller, &updated_member_info);

      Ok(())
    }

    fn is_member(&self, who: Option<AccountId>) -> bool {
      let who = who.unwrap_or(self.env().caller());
      let member_status = self.member_status(who);

      member_status == MemberStatus::Active || member_status == MemberStatus::Inactive
    }
  }
}
