use std::collections::HashMap;

use heimdall_common::{
    ether::signatures::{ResolvedError, ResolvedLog},
    io::logging::TraceFactory,
};

use super::Snapshot;

pub fn generate_and_write_contract_csv(
    snapshots: &Vec<Snapshot>,
    resolved_errors: &HashMap<String, ResolvedError>,
    resolved_events: &HashMap<String, ResolvedLog>,
    output_path: &str,
) {
}
