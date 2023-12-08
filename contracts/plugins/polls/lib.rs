#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use polls::{PollsRef};

#[ink::contract]
mod polls {
  use ink::env::call::{build_call, ExecutionInput, Selector};
  use ink::env::DefaultEnvironment;
  use ink::prelude::{string::String, format, vec::Vec};
  use ink::storage::{Mapping, Lazy};

  type Result<T> = core::result::Result<T, Error>;
  type Nonce = u32;
  type PollId = u32;
  type OptionIndex = u32;

  #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq, Clone)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum Error {
    Custom(String),
    UnAuthorized,
    NotActiveMember,
    NotSpaceOwner,
    PollNotFound,
    InvalidOptionIndex,
    VoteNotFound,
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct Poll {
    title: String,
    desc: Option<String>,
    options: Vec<String>,
    author: AccountId,
    created_at: Timestamp,
    updated_at: Option<Timestamp>,
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct PollVotes {
    total_votes: u32,
    votes_by_options: Vec<(OptionIndex, u32)>,
    voted_option: Option<OptionIndex>
  }

  #[ink(storage)]
  #[derive(Default)]
  pub struct Polls {
    space_id: Lazy<AccountId>,
    launcher_id: Lazy<AccountId>,

    polls: Mapping<PollId, Poll>,
    polls_nonce: Lazy<Nonce>,

    votes_voters: Mapping<(PollId, AccountId), OptionIndex>,
    votes_counters: Mapping<(PollId, OptionIndex), u32>,
  }

  impl Polls {
    #[ink(constructor)]
    pub fn new(space_id: AccountId, launcher_id: AccountId) -> Self {
      let mut one = Polls::default();
      one.space_id.set(&space_id);
      one.launcher_id.set(&launcher_id);

      one
    }

    /// New poll
    #[ink(message)]
    pub fn new_poll(&mut self, title: String, desc: Option<String>, options: Vec<String>) -> Result<PollId> {
      // For now, only space owner can create poll
      self.ensure_space_owner()?;
      let new_poll_id = self.polls_nonce.get_or_default();
      let next_poll_id = new_poll_id.checked_add(1).expect("Exceeding number of polls!");

      let new_poll = Poll {
        title,
        desc,
        options,
        author: self.env().caller(),
        created_at: self.env().block_timestamp(),
        updated_at: None,
      };

      self.polls.insert(new_poll_id, &new_poll);
      self.polls_nonce.set(&next_poll_id);

      Ok(0)
    }
    /// Update poll
    #[ink(message)]
    pub fn update_poll(&mut self, poll_id: PollId, title: Option<String>,
                       desc: Option<String>, options: Option<Vec<String>>) -> Result<()> {
      self.ensure_space_owner()?;
      let mut poll = self.polls.get(poll_id).ok_or(Error::PollNotFound)?;

      if let Some(value) = title {
        poll.title = value;
      }

      poll.desc = desc;

      if let Some(values) = options {
        if values.len() < poll.options.len() {
          return Err(Error::Custom(String::from("Cannot remove option")));
        }

        poll.options = values;
      }

      self.polls.insert(poll_id, &poll);

      Ok(())
    }

    /// Get polls by ids
    #[ink(message)]
    pub fn polls_by_ids(&self, ids: Vec<PollId>) -> Vec<(PollId, Option<Poll>)> {
      ids.iter()
        .map(|&id| (id, self.polls.get(id)))
        .collect()
    }

    /// Polls count
    #[ink(message)]
    pub fn polls_count(&self) -> u32 {
      self.polls_nonce.get_or_default()
    }

    /// Get votes information of a poll
    #[ink(message)]
    pub fn poll_votes(&self, poll_id: PollId) -> Result<PollVotes> {
      let poll = self.polls.get(poll_id).ok_or(Error::PollNotFound)?;
      let mut total_votes: u32 = 0;
      let mut votes_by_options: Vec<(OptionIndex, u32)> = Vec::new();

      for index in 0..(poll.options.len()) {
        let option_index = index as u32;
        let votes_by_option = self.votes_counters.get((poll_id, option_index)).unwrap_or_default();
        total_votes = total_votes.saturating_add(votes_by_option);
        votes_by_options.push((option_index, votes_by_option));
      }

      let caller = self.env().caller();
      let voted_option = self.votes_voters.get((poll_id, caller));

      Ok(PollVotes {
        total_votes,
        votes_by_options,
        voted_option,
      })
    }

    /// Vote
    #[ink(message)]
    pub fn vote(&mut self, poll_id: PollId, option_index: OptionIndex) -> Result<()> {
      self.ensure_active_member()?;
      let poll = self.polls.get(poll_id).ok_or(Error::PollNotFound)?;
      let _ = poll.options.get(option_index as usize).ok_or(Error::InvalidOptionIndex)?;

      let voter = self.env().caller();
      let maybe_voted_option = self.votes_voters.get((poll_id, voter));
      if let Some(voted_option) = maybe_voted_option {
        let votes_counter = self.votes_counters.get((poll_id, voted_option)).unwrap_or_default();
        self.votes_counters.insert((poll_id, voted_option), &votes_counter.saturating_sub(1));
      }

      let new_votes_counter = self.votes_counters.get((poll_id, option_index)).unwrap_or_default();
      self.votes_counters.insert((poll_id, option_index), &new_votes_counter.saturating_add(1));
      self.votes_voters.insert((poll_id, voter), &option_index);


      Ok(())
    }
    /// UnVote
    #[ink(message)]
    pub fn unvote(&mut self, poll_id: PollId) -> Result<()> {
      self.ensure_active_member()?;
      let _ = self.polls.get(poll_id).ok_or(Error::PollNotFound)?;
      let voter = self.env().caller();
      let voted_option = self.votes_voters.get((poll_id, voter)).ok_or(Error::VoteNotFound)?;
      self.votes_voters.remove((poll_id, voter));

      let votes_counter = self.votes_counters.get((poll_id, voted_option)).unwrap_or_default();
      self.votes_counters.insert((poll_id, voted_option), &votes_counter.saturating_sub(1));

      Ok(())
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
        .map_err(|err| Error::Custom(format!("Failed to `set_code_hash` to {:?} due to {:?}", code_hash, err)))?;

      ::ink::env::debug_println!("Switched code hash to {:?}.", code_hash);

      Ok(())
    }

    #[ink(message)]
    pub fn code_hash(&self) -> Hash {
      self.env().code_hash(&self.env().account_id()).unwrap()
    }
  }
}
