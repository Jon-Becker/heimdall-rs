use alloy_json_abi::JsonAbi;
use heimdall_common::utils::io::logging::TraceFactory;

use crate::{error::Error, interfaces::DecompilerArgs};

#[derive(Debug, Clone)]
pub struct DecompileResult {
    pub source: Option<String>,
    pub abi: JsonAbi,
    _trace: TraceFactory,
}

impl DecompileResult {
    pub fn display(&self) {
        self._trace.display();
    }
}

pub async fn decompile(args: DecompilerArgs) -> Result<DecompileResult, Error> {
    todo!()
}
