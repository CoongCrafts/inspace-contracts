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

  /// Who can post?
  #[derive(Clone, Default, Debug, scale::Decode, scale::Encode)]
  #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
  pub enum PostPerm {
    #[default]
    SpaceOwner,
    ActiveMember,
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
    pub fn new_post(&mut self, content: PostContent) -> PostResult<PostId> {
      self.ensure_post_permission()?;

      let caller = Self::env().caller();

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

    /// TODO list_posts

    fn get_post_by_id(&self, id: PostId) -> Option<Post> {
      self.posts.get(id)
    }

    fn ensure_post_permission(&self) -> PostResult<()> {
      let permission = self.post_perm();

      match permission {
        PostPerm::SpaceOwner => Ok(self._ensure_space_owner()?),
        PostPerm::ActiveMember => Ok(self._ensure_active_member()?),
      }
    }
  }
}
