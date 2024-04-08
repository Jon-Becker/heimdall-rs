use std::collections::HashMap;

use heimdall_common::{
    ether::signatures::{ResolvedError, ResolvedLog},
    utils::io::logging::TraceFactory,
};

use crate::{
    error::Error,
    interfaces::{Snapshot, SnapshotArgs},
};

#[derive(Debug, Clone)]
pub struct SnapshotResult {
    pub snapshots: Vec<Snapshot>,
    pub resolved_errors: HashMap<String, ResolvedError>,
    pub resolved_events: HashMap<String, ResolvedLog>,
    _trace: TraceFactory,
}

impl SnapshotResult {
    pub fn display(&self) {
        self._trace.display();
    }

    pub fn generate_csv(&self) -> Vec<String> {
        todo!()
    }
}

pub async fn snapshot(_args: SnapshotArgs) -> Result<SnapshotResult, Error> {
    todo!()
}
