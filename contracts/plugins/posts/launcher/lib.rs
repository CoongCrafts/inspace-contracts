#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[openbrush::implementation(Ownable, Upgradeable)]
#[openbrush::contract]
mod posts_launcher {
  use ink::ToAccountId;
  use ink::env::call::{build_create, ExecutionInput, Selector};
  use posts::{PostsRef};
  use openbrush::traits::Storage;
  use shared::traits::codehash::*;
  use shared::traits::plugin_launcher::*;

  #[ink(storage)]
  #[derive(Storage, Default)]
  pub struct PostsLauncher {
    #[storage_field]
    ownable: ownable::Data,
    #[storage_field]
    launcher: plugin_launcher::Data
  }

  impl plugin_launcher::Instantiator for PostsLauncher {
    fn _initiate_new_plugin(&self, space_id: AccountId, launcher_id: AccountId, salt: &[u8]) -> Result<AccountId, LauncherError> {
      let input =
        ExecutionInput::new(Selector::new(ink::selector_bytes!("new")))
          .push_arg(space_id)
          .push_arg(launcher_id);

      let new_contract: PostsRef = build_create::<PostsRef>()
        .code_hash(self.latest_plugin_code())
        .gas_limit(0)
        .endowment(0)
        .exec_input(input)
        .salt_bytes(salt)
        .returns::<PostsRef>()
        .instantiate();

      Ok(new_contract.to_account_id())
    }
  }

  impl CodeHash for PostsLauncher {}
  impl PluginLauncher for PostsLauncher {}

  impl PostsLauncher {
    #[ink(constructor)]
    pub fn new(motherspace_id: AccountId, owner_id: AccountId, plugin_code: Hash) -> Self {
      let mut one = Self::default();
      plugin_launcher::PluginLauncher::_init(&mut one, motherspace_id, plugin_code);
      ownable::Internal::_init_with_owner(&mut one, owner_id);

      one
    }
  }
}
