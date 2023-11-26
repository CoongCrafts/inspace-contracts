#![cfg_attr(not(feature = "std"), no_std, no_main)]

/// Ref: https://github.com/InkSmartContract/BlockchainFoodOrder/blob/main/contracts/foodorder/logic/helpers/helpers.rs
#[macro_export]
macro_rules! ensure {
    ( $x:expr, $y:expr $(,)? ) => {{
        if !$x {
            return Err($y.into());
        }
    }};
}