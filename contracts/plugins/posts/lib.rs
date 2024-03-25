#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use posts::{PostsRef};

#[openbrush::contract]
mod posts {
  use ink::prelude::{vec::Vec};
  use ink::storage::{Mapping, Lazy};
  use openbrush::{modifiers, traits::{Storage, String}};
  use shared::traits::codehash::*;
  use shared::traits::plugin_base::*;

  type PostResult<T> = core::result::Result<T, PostError>;

  #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq, Clone)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum PostError {
    Custom(String),
    PluginError(PluginError),
    PostNotExisted,
  }

  impl From<PluginError> for PostError {
    fn from(error: PluginError) -> Self {
      PostError::PluginError(error)
    }
  }

  type PostId = u32;
  type Nonce = u32;

  pub type PendingPostApproval = (PostId, bool);

  #[derive(Clone, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub struct ApprovalSubmissionResult {
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
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
  pub enum PostCreationStatus {
    Created,
    Pending,
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
  #[derive(Default, Storage)]
  pub struct Posts {
    #[storage_field]
    base: plugin_base::Data,

    posts: Mapping<PostId, Post>,
    posts_nonce: Lazy<Nonce>,

    pending_posts: Mapping<PostId, Post>,
    author_to_pending_posts: Mapping<AccountId, Vec<PostId>>,
    pending_post_ids: Lazy<Vec<PostId>>,
    pending_posts_nonce: Lazy<Nonce>,

    pinned_posts: Lazy<Vec<PostId>>,

    post_perm: Lazy<PostPerm>,
  }

  impl CodeHash for Posts {}
  impl PluginBase for Posts {}

  impl Posts {
    #[ink(constructor)]
    pub fn new(space_id: AccountId, launcher_id: AccountId) -> Self {
      let mut one = Self::default();
      plugin_base::PluginBase::_init(&mut one, space_id, launcher_id);

      one
    }

    #[ink(message)]
    pub fn new_post(&mut self, content: PostContent) -> PostResult<(PostCreationStatus, u32)> {
      self.ensure_post_permission()?;

      // TODO verify post content

      let caller = Self::env().caller();
      let permission = self.post_perm();

      Ok(match permission {
        PostPerm::SpaceOwner | PostPerm::ActiveMember => (PostCreationStatus::Created, self._new_post(content)?),
        PostPerm::ActiveMemberWithApproval => {
          let space_owner = self.get_space_owner_id();

          if caller == space_owner {
            (PostCreationStatus::Created, self._new_post(content)?)
          } else {
            (PostCreationStatus::Pending, self._new_pending_post(content)?)
          }
        }
      })
    }

    #[ink(message)]
    pub fn list_pending_posts(&self, from: u32, per_page: u32) -> PostsPage {
      let per_page = per_page.min(50); // limit per page at max 50 items
      let posts = self.pending_post_ids.get_or_default();
      let last_position = from.saturating_add(per_page);
      let total = posts.len() as u32;

      let page: Option<&[PostId]> = posts.get((from as usize)..(last_position.min(total) as usize));
      let items = match page {
        Some(list) => list.iter()
          .map(|id| PostRecord {
            post_id: *id,
            post: self.pending_posts.get(id).unwrap(),
          })
          .collect(),
        None => Vec::new()
      };

      PostsPage {
        items,
        from,
        per_page,
        has_next_page: last_position < total,
        total,
      }
    }

    #[ink(message)]
    pub fn pending_posts_by_author(&self, who: Option<AccountId>) -> Vec<PostRecord> {
      let author = who.unwrap_or(self.env().caller());
      let pending_post_ids = self.author_to_pending_posts.get(author).unwrap_or_default();

      pending_post_ids.iter()
        .map(|&id| PostRecord {
          post_id: id,
          post: self.pending_posts.get(id).unwrap(),
        })
        .collect()
    }

    #[ink(message)]
    pub fn submit_pending_post_approvals(&mut self, approvals: Vec<PendingPostApproval>) -> PostResult<ApprovalSubmissionResult> {
      self.ensure_space_owner()?;

      let mut approved_count: u32 = 0;
      let mut rejected_count: u32 = 0;
      let mut not_found_count: u32 = 0;

      let mut submitted_posts_id: Vec<u32> = Vec::new();
      for approval in approvals {
        let (pending_post_id, approved) = approval;

        if let Some(pending_post) = self.pending_posts.get(pending_post_id) {
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

      let mut pending_posts = self.pending_post_ids.get_or_default();
      pending_posts.retain(|id| !submitted_posts_id.contains(id));
      self.pending_post_ids.set(&pending_posts);

      for post_id in submitted_posts_id {
        let submitted_post = self.pending_posts.get(post_id).unwrap();
        let mut author_to_id = self.author_to_pending_posts.get(submitted_post.author).unwrap();
        author_to_id.retain(|id| id != &post_id);
        self.author_to_pending_posts.insert(submitted_post.author, &author_to_id);

        self.pending_posts.remove(post_id);
      }

      Ok(ApprovalSubmissionResult {
        approved: approved_count,
        rejected: rejected_count,
        not_found: not_found_count,
      })
    }

    #[ink(message)]
    pub fn pending_posts_count(&self) -> u32 {
      self.pending_post_ids.get_or_default().len() as u32
    }

    #[ink(message)]
    pub fn pending_posts_count_by_author(&self, who: Option<AccountId>) -> u32 {
      let author = who.unwrap_or(self.env().caller());

      self.author_to_pending_posts
        .get(author)
        .unwrap_or_default()
        .len() as u32
    }

    #[ink(message)]
    pub fn cancel_pending_post(&mut self, pending_post_id: PostId) -> PostResult<()> {
      self.ensure_active_member()?;

      let post = self.pending_posts.get(pending_post_id).ok_or(Error::PostNotExisted)?;

      let caller = Self::env().caller();
      if caller != post.author {
        return Err(Error::UnAuthorized);
      }

      let mut pending_posts = self.pending_post_ids.get_or_default();
      pending_posts.retain(|&x| x != pending_post_id);
      self.pending_post_ids.set(&pending_posts);
      self.pending_posts.remove(pending_post_id);

      let mut author_to_id = self.author_to_pending_posts.get(caller).unwrap();
      author_to_id.retain(|&id| id != pending_post_id);
      self.author_to_pending_posts.insert(caller, &author_to_id);

      Ok(())
    }

    #[ink(message)]
    pub fn update_pending_post(&mut self, pending_post_id: PostId, content: PostContent) -> PostResult<()> {
      self.ensure_active_member()?;

      let mut post = self.pending_posts.get(pending_post_id).ok_or(Error::PostNotExisted)?;

      let caller = Self::env().caller();
      if caller != post.author {
        return Err(Error::UnAuthorized);
      }

      post.content = content;
      self.pending_posts.insert(pending_post_id, &post);

      Ok(())
    }

    #[ink(message)]
    pub fn list_pinned_posts(&self) -> Vec<PostRecord> {
      let pinned_posts = self.pinned_posts.get_or_default();

      return pinned_posts.iter()
        .map(|&id| PostRecord {
          post_id: id,
          post: self.posts.get(id).unwrap(),
        })
        .collect();
    }

    #[ink(message)]
    pub fn pin_post(&mut self, post_id: PostId) -> PostResult<()> {
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
    pub fn unpin_post(&mut self, post_id: PostId) -> PostResult<()> {
      self.ensure_space_owner()?;

      let mut pinned_posts = self.pinned_posts.get_or_default();
      if pinned_posts.contains(&post_id) {
        pinned_posts.retain(|&id| id != post_id);
        self.pinned_posts.set(&pinned_posts);
      }

      Ok(())
    }


    #[ink(message)]
    #[modifiers(only_active_member)]
    pub fn update_post(&mut self, id: PostId, content: PostContent) -> PostResult<()> {
      let mut post = self.get_post_by_id(id).ok_or(PostError::PostNotExisted)?;

      let caller = Self::env().caller();
      let space_owner_id = self._space_owner_id();

      if !(caller == post.author || caller == space_owner_id) {
        return Err(PluginError::UnAuthorized.into());
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
    #[modifiers(only_space_owner)]
    pub fn update_perm(&mut self, new_perm: PostPerm) -> PostResult<()> {

      self.post_perm.set(&new_perm);

      Ok(())
    }

    #[ink(message)]
    pub fn posts_count(&self) -> u32 {
      self.posts_nonce.get_or_default()
    }

    fn _new_post(&mut self, content: PostContent) -> PostResult<PostId> {
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

    fn _new_pending_post(&mut self, content: PostContent) -> PostResult<PostId> {
      let caller = self.env().caller();

      // Create a pending post
      let new_pending_post_id = self.pending_posts_nonce.get_or_default();
      let next_pending_post_nonce = new_pending_post_id.checked_add(1).expect("Exceeds number of pending posts!");

      let new_pending_post = Post {
        author: caller,
        content,
        created_at: Self::env().block_timestamp(),
        updated_at: None,
      };

      self.pending_posts.insert(new_pending_post_id, &new_pending_post);
      self.author_to_pending_posts.insert(caller, &vec![new_pending_post_id]);

      let mut pending_posts = self.pending_post_ids.get_or_default();
      pending_posts.push(new_pending_post_id);

      self.pending_post_ids.set(&pending_posts);
      self.pending_posts_nonce.set(&next_pending_post_nonce);

      Ok(new_pending_post_id)
    }

    fn get_post_by_id(&self, id: PostId) -> Option<Post> {
      self.posts.get(id)
    }

    fn ensure_post_permission(&self) -> PostResult<()> {
      let permission = self.post_perm();

      match permission {
        PostPerm::SpaceOwner => Ok(self._ensure_space_owner()?),
        PostPerm::ActiveMember | PostPerm::ActiveMemberWithApproval => Ok(self._ensure_active_member()?),
      }
    }
  }
}
