//! Installing the French vACC vATIS profiles.
//!
//! Separate from [`crate::pack_sync`] on purpose. Every `FileOp` there is
//! relative to the controller pack's `install_root`, and `GNG_OWNED_PATHS`
//! depends on that being true. vATIS profiles live in the user's application
//! data, well outside that root, so they get their own root and their own
//! plan/apply pair rather than widening `pack_sync`'s.
//!
//! # Why the installer touches these at all
//!
//! vATIS refreshes profile *content* on its own — but its updater is
//! identity-preserving (`updatedProfile.Id = localProfile.Id`), and the profile
//! file is named after the id. When the upstream repository reissued every id
//! on 2026-09-04, no amount of waiting could migrate a controller onto the new
//! ones. That one-time migration is what this module performs; afterwards
//! vATIS's own updater takes over again and this module goes quiet.

pub mod apply;
pub mod paths;
pub mod plan;
pub mod profile;

pub use apply::{apply, VatisSummary};
pub use paths::{
    app_data_dir, backup_dir, detect_client, linux_client_install_path, profile_path,
    profiles_dir, Platform, APP_ID,
};
pub use plan::{plan, read_store, CanonicalProfile, ProfileOp, StoredProfile, VatisPlan};
pub use profile::{parse_header, ProfileHeader, ID_REISSUE_SERIAL};
