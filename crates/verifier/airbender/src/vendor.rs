use airbender_execution_utils::{
    setups::CompiledCircuitsSet,
    unified_circuit::flatten_proof_into_responses_for_unified_recursion,
    unrolled::{UnrolledProgramProof, UnrolledProgramSetup},
};
use airbender_full_statement_verifier::unified_circuit_statement::verify_unrolled_or_unified_circuit_recursion_layer;
use airbender_verifier_common::{SecurityModel, prover};

// Vendored from `airbender_execution_utils::unified_circuit::verify_proof_in_unified_layer`
// to strip the upstream `println!` calls.
pub fn verify_proof_in_unified_layer(
    proof: &UnrolledProgramProof,
    setup: &UnrolledProgramSetup,
    compiled_layouts: &CompiledCircuitsSet,
    input_is_unrolled: bool,
    security: SecurityModel,
) -> Result<[u32; 16], ()> {
    let responses = flatten_proof_into_responses_for_unified_recursion(
        proof,
        setup,
        compiled_layouts,
        input_is_unrolled,
    );

    #[cfg(target_arch = "wasm32")]
    {
        std::panic::catch_unwind(move || verify(responses, security)).map_err(|_| ())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::Builder::new()
            .name("verifier thread".to_string())
            .stack_size(1 << 27)
            .spawn(move || verify(responses, security))
            .expect("must spawn verifier thread")
            .join()
            .map_err(|_| ())
    }
}

#[inline]
fn verify(responses: Vec<u32>, security: SecurityModel) -> [u32; 16] {
    prover::nd_source_std::set_iterator(responses.into_iter());
    verify_unrolled_or_unified_circuit_recursion_layer(security)
}
