use crate::{
    error::Error,
    interfaces::{DumpArgs, DumpRow},
};

pub async fn dump(_args: DumpArgs) -> Result<Vec<DumpRow>, Error> {
    todo!()
}
