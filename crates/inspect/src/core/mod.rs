use heimdall_common::utils::io::logging::TraceFactory;

use crate::{error::Error, interfaces::InspectArgs};

#[derive(Debug, Clone)]
pub struct InspectResult {
    pub decoded_trace: Option<u8>, //DecodedTransactionTrace
    _trace: TraceFactory,
}

impl InspectResult {
    pub fn display(&self) {
        self._trace.display();
    }
}

pub async fn inspect(args: InspectArgs) -> Result<InspectResult, Error> {
    todo!()
}
