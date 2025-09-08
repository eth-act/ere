use crate::ErezkVM;
use serde::de::DeserializeOwned;
use std::io::Read;
use zkvm_interface::zkVMError;

#[path = "../../ere-risc0/src/output.rs"]
mod ere_risc0_output;

impl ErezkVM {
    pub fn deserialize_from<R: Read, T: DeserializeOwned>(
        &self,
        reader: R,
    ) -> Result<T, zkVMError> {
        match self {
            // Issue for tracking: https://github.com/eth-act/ere/issues/4.
            Self::Jolt => todo!(),
            // Issue for tracking: https://github.com/eth-act/ere/issues/63.
            Self::Nexus => todo!(),
            Self::OpenVM => unimplemented!("no native serialization in this platform"),
            Self::Pico => bincode::deserialize_from(reader).map_err(zkVMError::other),
            Self::Risc0 => ere_risc0_output::deserialize_from(reader),
            Self::SP1 => bincode::deserialize_from(reader).map_err(zkVMError::other),
            Self::Ziren => bincode::deserialize_from(reader).map_err(zkVMError::other),
            Self::Zisk => unimplemented!("no native serialization in this platform"),
        }
    }
}
