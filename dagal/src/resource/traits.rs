use std::fmt::Debug;
use crate::traits::Destructible;

/// Every resource in Vulkan is expected to have a lifetime + debuggable
pub trait Resource<'a>: Destructible + Debug {
	/// Get the name of the resource
	fn get_name() -> &'a str;
}