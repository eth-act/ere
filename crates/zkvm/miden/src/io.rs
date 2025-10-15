use crate::error::{ExecuteError, MidenError};
use ere_zkvm_interface::{Input, InputItem, PublicValues};
use miden_processor::{AdviceInputs, StackInputs, StackOutputs};

/// Returns Miden compatible inputs from `ere_zkvm_interface::Input`.
///
/// All inputs are serialized and concatenated, then placed onto the advice tape.
/// The stack is left empty.
pub fn generate_miden_inputs(inputs: &Input) -> Result<(StackInputs, AdviceInputs), MidenError> {
    let mut all_bytes = Vec::new();

    for item in inputs.iter() {
        match item {
            InputItem::Object(obj) => {
                bincode::serialize_into(&mut all_bytes, &**obj)
                    .map_err(ExecuteError::Serialization)?;
            }
            InputItem::SerializedObject(bytes) | InputItem::Bytes(bytes) => {
                all_bytes.extend_from_slice(bytes);
            }
        }
    }

    // Convert the byte stream into u64 words for the Miden VM.
    let advice_words: Vec<u64> = {
        let mut words: Vec<u64> = all_bytes
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
            .collect();

        let remainder = all_bytes.chunks_exact(8).remainder();
        if !remainder.is_empty() {
            let mut last_chunk = [0u8; 8];
            last_chunk[..remainder.len()].copy_from_slice(remainder);
            words.push(u64::from_le_bytes(last_chunk));
        }

        words
    };

    let advice_inputs = AdviceInputs::default()
        .with_stack_values(advice_words)
        .map_err(|e| ExecuteError::InvalidInput(e.to_string()))?;

    Ok((StackInputs::default(), advice_inputs))
}

// Convert Miden stack outputs to public values
pub fn outputs_to_public_values(outputs: &StackOutputs) -> Result<PublicValues, bincode::Error> {
    let output_ints: Vec<u64> = outputs.iter().map(|f| f.as_int()).collect();
    bincode::serialize(&output_ints)
}
