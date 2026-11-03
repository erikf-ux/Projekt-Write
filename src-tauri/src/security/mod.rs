//! Security checks shared by all services (section 20).
//!
//! * [`paths`]: library-relative path resolution that rejects traversal,
//!   reserved names and symbolic links.

pub mod paths;
