use heimdall_common::{ether::signatures::ResolvedFunction, utils::io::logging::TraceFactory};

use crate::{error::Error, interfaces::DecodeArgs};

#[derive(Debug, Clone)]
pub struct DecodeResult {
    pub decoded: Vec<ResolvedFunction>,
    _trace: TraceFactory,
}

impl DecodeResult {
    pub fn display(&self) {
        self._trace.display();
    }
}

pub async fn decode(args: DecodeArgs) -> Result<DecodeResult, Error> {
    todo!()
}
