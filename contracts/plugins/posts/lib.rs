#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use posts::{PostsRef};

#[ink::contract]
mod posts {
  use ink::env::call::{build_call, ExecutionInput, Selector};
  use ink::env::DefaultEnvironment;
  use ink::prelude::{vec::Vec, string::String, vec};
  use ink::storage::{Lazy, Mapping};

  type Result<T> = core::result::Result<T, Error>;

  #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq, Clone)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum Error {
    Custom(String),
    UnAuthorized,
    PostNotExisted,
    NotActiveMember,
    NotSpaceOwner,
  }

  type PostId = u32;
  type Nonce = u32;
  type PendingPostId = u32;

  pub type PendingPostApproval = (PendingPostId, bool);

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct ApprovalSubmissionResult  {
    approved: u32,
    rejected: u32,
    not_found: u32,
  }

  /// Who can post?
  #[derive(Clone, Default, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub enum PostPerm {
    #[default]
    SpaceOwner,
    ActiveMember,
    ActiveMemberWithApproval,
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub enum PostContent {
    Raw(String),
    IpfsCid(String),
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

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub enum Ordering {
    Descending,
    Ascending,
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub struct Post {
    content: PostContent,
    author: AccountId,
    created_at: Timestamp,
    updated_at: Option<Timestamp>,
  }

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct PostRecord {
    post_id: PostId,
    post: Post,
  }

  type PostsPage = Pagination<PostRecord>;

  #[ink(storage)]
  #[derive(Default)]
  pub struct Posts {
    space_id: Lazy<AccountId>,
    launcher_id: Lazy<AccountId>,

    posts: Mapping<PostId, Post>,
    posts_nonce: Lazy<Nonce>,

    pending_posts_list: Mapping<PendingPostId, Post>,
    author_to_pending_posts: Mapping<AccountId, Vec<PendingPostId>>,
    pending_posts: Lazy<Vec<PendingPostId>>,
    pending_posts_nonce: Lazy<Nonce>,

    pinned_posts: Lazy<Vec<PostId>>,

    post_perm: Lazy<PostPerm>,
  }

  impl Posts {
    #[ink(constructor)]
    pub fn new(space_id: AccountId, launcher_id: AccountId) -> Self {
      let mut one = Posts::default();
      one.space_id.set(&space_id);
      one.launcher_id.set(&launcher_id);

      one
    }

    #[ink(message)]
    pub fn new_post(&mut self, content: PostContent) -> Result<PostId> {
      self.ensure_post_permission()?;

      let caller = Self::env().caller();
      let permission = self.post_perm();

      match permission {
        PostPerm::SpaceOwner | PostPerm::ActiveMember => {
          return self.create_post(content);
        },
        PostPerm::ActiveMemberWithApproval => {
          let space_owner = self.get_space_owner_id();

          if caller == space_owner {
            return self.create_post(content);
          } else {
            let new_pending_post_id = self.pending_posts_nonce.get_or_default();
            let next_pending_post_none = new_pending_post_id.checked_add(1).expect("Exceeds number of pending posts!");

            let new_pending_post = Post {
              author: caller,
              content,
              created_at: Self::env().block_timestamp(),
              updated_at: None,
            };

            self.pending_posts_list.insert(new_pending_post_id, &new_pending_post);
            self.author_to_pending_posts.insert(caller, &vec![new_pending_post_id]);

            let mut pending_posts = self.pending_posts.get_or_default();
            pending_posts.push(new_pending_post_id);

            self.pending_posts.set(&pending_posts);
            self.pending_posts_nonce.set(&next_pending_post_none);

            Ok(new_pending_post_id)
          }
        }
      }
    }

    #[ink(message)]
    pub fn list_pending_posts(&self, from: u32, per_page: u32) -> PostsPage {
      self.ensure_space_owner().expect("NotSpaceOwner");

      let per_page = per_page.min(50); // limit per page at max 50 items
      let posts = self.pending_posts.get_or_default();
      let last_position = from.saturating_add(per_page);
      let total = posts.len() as u32;

      let page: Option<&[PendingPostId]> = posts.get((from as usize)..(last_position.min(total) as usize));
      let items  = match page {
        Some(list) => list.iter().map(|id| PostRecord {post_id: *id, post:self.pending_posts_list.get(id).unwrap()}).collect(),
        None => Vec::new()
      };

      return PostsPage {
        items,
        from,
        per_page,
        has_next_page: last_position < total,
        total,
      }
    }

    #[ink(message)]
    pub fn pending_posts_by_author(&self, who: Option<AccountId>) -> Result<Vec<PostRecord>> {
      self.ensure_active_member()?;

      let caller = self.env().caller();
      let space_owner_id = self.get_space_owner_id();
      let target = who.unwrap_or(caller);

      if caller != target && caller != space_owner_id {
        return Err(Error::UnAuthorized);
      }

      let pending_posts = self.author_to_pending_posts.get(target);
      let items = match pending_posts {
        Some(list) => list.iter().map(|id| PostRecord {post_id: *id, post:self.pending_posts_list.get(id).unwrap()}).collect(),
        None => Vec::new()
      };

      Ok(items)
    }

    #[ink(message)]
    pub fn submit_pending_post_approvals(&mut self, approvals: Vec<PendingPostApproval>) -> Result<ApprovalSubmissionResult> {
      self.ensure_space_owner()?;

      let mut approved_count: u32 = 0;
      let mut rejected_count: u32 = 0;
      let mut not_found_count: u32 = 0;

      let mut submitted_posts_id: Vec<u32> = Vec::new();
      for approval in approvals {
        let (pending_post_id, approved) = approval;

        if let Some(pending_post) = self.pending_posts_list.get(pending_post_id) {
          submitted_posts_id.push(pending_post_id);

          if approved {
            let new_post_id = self.posts_nonce.get_or_default();
            let next_post_nonce = new_post_id.checked_add(1).expect("Exceeds number of posts!");

            self.posts.insert(new_post_id, &pending_post);
            self.posts_nonce.set(&next_post_nonce);

            approved_count = approved_count.saturating_add(1);
          } else {
            rejected_count = rejected_count.saturating_add(1);
          }
        } else {
          not_found_count = not_found_count.saturating_add(1);
        }
      }

      let mut pending_posts = self.pending_posts.get_or_default();
      pending_posts.retain(|id| !submitted_posts_id.contains(id));
      self.pending_posts.set(&pending_posts);

      for post_id in submitted_posts_id {
        let submitted_post = self.pending_posts_list.get(post_id).unwrap();
        let mut author_to_id = self.author_to_pending_posts.get(submitted_post.author).unwrap();
        author_to_id.retain(|id| id != &post_id);
        self.author_to_pending_posts.insert(submitted_post.author, &author_to_id);

        self.pending_posts_list.remove(post_id);
      }

      Ok(ApprovalSubmissionResult {
        approved: approved_count,
        rejected: rejected_count,
        not_found: not_found_count,
      })
    }

    #[ink(message)]
    pub fn pending_posts_count(&self) -> Result<u32> {
      self.ensure_space_owner()?;

      Ok(self.pending_posts.get_or_default().len() as u32)
    }

    #[ink(message)]
    pub fn pending_posts_count_by_author(&self, who: Option<AccountId>) -> Result<u32> {
      self.ensure_active_member()?;

      let caller = self.env().caller();
      let space_owner_id = self.get_space_owner_id();
      let target = who.unwrap_or(caller);

      if caller != target && caller != space_owner_id {
        return Err(Error::UnAuthorized);
      }

      let pending_posts = self.author_to_pending_posts.get(target).unwrap_or_default();

      Ok(pending_posts.len() as u32)
    }

    #[ink(message)]
    pub fn cancel_pending_post(&mut self, pending_post_id: PendingPostId) -> Result<()> {
        self.ensure_active_member()?;

        let post = self.pending_posts_list.get(pending_post_id).ok_or(Error::PostNotExisted)?;

        let caller = Self::env().caller();
        if caller != post.author {
          return Err(Error::UnAuthorized);
        }

        let mut pending_posts = self.pending_posts.get_or_default();
        pending_posts.retain(|x| x != &pending_post_id);
        self.pending_posts.set(&pending_posts);
        self.pending_posts_list.remove(pending_post_id);

        let mut author_to_id = self.author_to_pending_posts.get(caller).unwrap();
        author_to_id.retain(|id| id != &pending_post_id);
        self.author_to_pending_posts.insert(caller, &author_to_id);

        Ok(())
    }

    #[ink(message)]
    pub fn update_pending_post(&mut self, pending_post_id: PendingPostId, content: PostContent) -> Result<()> {
      self.ensure_active_member()?;

      let mut post = self.pending_posts_list.get(pending_post_id).ok_or(Error::PostNotExisted)?;

      let caller = Self::env().caller();
      if caller != post.author {
        return Err(Error::UnAuthorized);
      }

      post.content = content;
      self.pending_posts_list.insert(pending_post_id, &post);

      Ok(())
    }

    #[ink(message)]
    pub fn list_pinned_posts(&self) -> Vec<PostRecord> {
      let pinned_posts = self.pinned_posts.get_or_default();

      return pinned_posts.iter().map(|id| PostRecord {post_id: *id, post: self.posts.get(id).unwrap()}).collect();
    }

    #[ink(message)]
    pub fn pin_post(&mut self, post_id: PostId) -> Result<()> {
      self.ensure_space_owner()?;

      if !self.posts.contains(post_id) {
        return Err(Error::PostNotExisted);
      }

      let mut pinned_posts = self.pinned_posts.get_or_default();
      if !pinned_posts.contains(&post_id) {
        pinned_posts.push(post_id);
      }

      self.pinned_posts.set(&pinned_posts);

      Ok(())
    }

    #[ink(message)]
    pub fn unpin_post(&mut self, post_id: PostId) -> Result<()> {
      self.ensure_space_owner()?;

      let mut pinned_posts = self.pinned_posts.get_or_default();
      if pinned_posts.contains(&post_id) {
        pinned_posts.retain(|id| id != &post_id);
      }

      self.pinned_posts.set(&pinned_posts);

      Ok(())
    }


    #[ink(message)]
    pub fn update_post(&mut self, id: PostId, content: PostContent) -> Result<()> {
      self.ensure_active_member()?;

      let mut post = self.get_post_by_id(id).ok_or(Error::PostNotExisted)?;

      let caller = Self::env().caller();
      let space_owner_id = self.get_space_owner_id();

      if !(caller == post.author || caller == space_owner_id) {
        return Err(Error::UnAuthorized);
      }

      post.content = content;
      post.updated_at = Some(Self::env().block_timestamp());

      self.posts.insert(id, &post);

      Ok(())
    }

    #[ink(message)]
    pub fn list_posts(&self, from: u32, per_page: u32, ordering: Ordering) -> PostsPage {
      match ordering {
        Ordering::Ascending => panic!("Not supported"),
        Ordering::Descending => {
          let per_page = per_page.min(50); // limit per page at max 50 items
          let current_posts_nonce = self.posts_nonce.get_or_default();
          let bounded_from = from.saturating_add(1);
          let last_position = bounded_from.saturating_sub(per_page);

          let mut post_records = Vec::new();
          for index in ((last_position as usize)..(bounded_from.min(current_posts_nonce) as usize)).rev() {
            let bounded_index = index as u32;

            if let Some(post) = self.posts.get(bounded_index) {
              post_records.push(PostRecord { post_id: bounded_index, post });
            }
          }

          PostsPage {
            items: post_records,
            from,
            per_page,
            has_next_page: last_position > 0,
            total: current_posts_nonce,
          }
        }
      }
    }

    #[ink(message)]
    pub fn post_by_id(&self, id: PostId) -> Option<Post> {
      self.get_post_by_id(id)
    }

    #[ink(message)]
    pub fn posts_by_ids(&self, ids: Vec<PostId>) -> Vec<(PostId, Option<Post>)> {
      ids.iter()
        .map(|&id| (id, self.get_post_by_id(id)))
        .collect()
    }

    #[ink(message)]
    pub fn post_perm(&self) -> PostPerm {
      self.post_perm.get_or_default()
    }

    #[ink(message)]
    pub fn update_perm(&mut self, new_perm: PostPerm) -> Result<()> {
      self.ensure_space_owner()?;

      self.post_perm.set(&new_perm);

      Ok(())
    }

    #[ink(message)]
    pub fn posts_count(&self) -> u32 {
      self.posts_nonce.get_or_default()
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

    fn create_post(&mut self, content: PostContent) -> Result<PostId> {
      self.ensure_post_permission()?;

      let caller = self.env().caller();

      let new_post_id = self.posts_nonce.get_or_default();
      let next_post_nonce = new_post_id.checked_add(1).expect("Exceeds number of posts!");

      let new_post = Post {
        author: caller,
        content,
        created_at: Self::env().block_timestamp(),
        updated_at: None,
      };

      self.posts.insert(new_post_id, &new_post);
      self.posts_nonce.set(&next_post_nonce);

      Ok(new_post_id)
    }

    fn get_post_by_id(&self, id: PostId) -> Option<Post> {
      self.posts.get(id)
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

    fn get_space_owner_id(&self) -> AccountId {
      build_call::<DefaultEnvironment>()
        .call(self.get_space_id())
        .gas_limit(0)
        .exec_input(
          ExecutionInput::new(Selector::new(ink::selector_bytes!("owner_id")))
        )
        .returns::<AccountId>()
        .invoke()
    }

    fn ensure_space_owner(&self) -> Result<()> {
      let caller = Self::env().caller();
      let space_owner_id = self.get_space_owner_id();

      if space_owner_id == caller {
        Ok(())
      } else {
        Err(Error::NotSpaceOwner)
      }
    }

    fn ensure_post_permission(&self) -> Result<()> {
      let permission = self.post_perm();

      match permission {
        PostPerm::SpaceOwner => self.ensure_space_owner(),
        PostPerm::ActiveMember | PostPerm::ActiveMemberWithApproval => self.ensure_active_member(),
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
