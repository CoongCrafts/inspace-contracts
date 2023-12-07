#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use posts::{PostsRef};

#[ink::contract]
mod posts {
  use ink::env::call::{build_call, ExecutionInput, Selector};
  use ink::env::DefaultEnvironment;
  use ink::prelude::{vec::Vec, string::String, format};
  use ink::storage::{Mapping, Lazy};

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
      self.ensure_active_member()?;

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
    pub fn update_post(&mut self, id: PostId, content: PostContent) -> Result<()> {
      let caller = Self::env().caller();

      self.ensure_active_member()?;

      let mut post = self.get_post_by_id(id).ok_or(Error::PostNotExisted)?;

      // TODO allow space owner to update post
      if caller != post.author {
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
              post_records.push(PostRecord {post_id: bounded_index, post});
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
    pub fn posts_count(&self) -> u32 {
      self.posts_nonce.get_or_default()
    }

    /// TODO list_posts

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
