pub mod airac;
pub mod apply;
pub mod ownership;
pub mod plan;

pub use apply::apply;
pub use plan::{plan, AreaSource, FileOp, PlanInputs, SectorExt, SyncPlan, SyncSummary};

pub const SECTOR_BACKUP_DIRNAME: &str = "Backup";
pub const SECTORS_SUBPATH: &str = "LFXX/Sectors";
pub const CURRENT_AIRAC_FILE: &str = "LFXX/Sectors/current_airac.txt";
pub const INSTALLER_VERSION_FILE: &str = ".github/installer-version.txt";
pub const COPYRIGHT_FILE: &str = "aeronav_copyright.txt";

#[cfg(test)]
mod integration_test;
