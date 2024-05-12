use std::fmt::Debug;
use crate::traits::Destructible;
use anyhow::Result;

/// Every resource in Vulkan is expected to have a lifetime + debuggable
pub trait Resource<'a>: Destructible + Debug + Sized {
	type CreateInfo;

	/// Attempt to create a new resource given the [`Self::CreateInfo`] struct
	fn new(create_info: Self::CreateInfo) -> Result<Self>;

	/// Get the name of the resource
	fn get_name() -> &'a str;
}