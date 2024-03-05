use heimdall_common::utils::io::logging::TraceFactory;
use petgraph::Graph;

use crate::{error::Error, interfaces::CFGArgs};

#[derive(Debug, Clone)]
pub struct CFGResult {
    pub graph: Graph<String, String>,
    _trace: TraceFactory,
}

impl CFGResult {
    pub fn display(&self) {
        self._trace.display();
    }

    pub fn as_dot(&self) -> String {
        todo!()
    }
}

pub async fn cfg(args: CFGArgs) -> Result<CFGResult, Error> {
    todo!()
}
