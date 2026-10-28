//! Consolidated integration test suite for the OpenChat local service.
//!
//! Combining all integration test modules under a single test harness reduces duplicate
//! linking overhead, `.pdb` symbol bloat, and compilation time.

#[cfg(feature = "desktop-profile-test-support")]
#[path = "integration/process_rpc.rs"]
mod process_rpc;

#[cfg(all(windows, feature = "storage-broker-test-support"))]
#[path = "integration/storage_broker_appcontainer.rs"]
mod storage_broker_appcontainer;
