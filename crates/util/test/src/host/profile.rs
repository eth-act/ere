use std::collections::BTreeSet;

use ere_prover_core::{CostEstimation, CostProfile, Elf, zkVMProver};

use crate::{
    host::TestCase,
    program::zkvm_interface::{self, Accelerator},
};

pub fn run_zkvm_profile(zkvm: &impl zkVMProver, test_case: &impl TestCase) -> CostProfile {
    let input = test_case.input();
    let (public_values, profile) = zkvm
        .profile(&input)
        .expect("profile should not fail with valid input");
    let (_, estimation) = zkvm
        .execute_estimated_cost(&input)
        .expect("execute_estimated_cost should not fail with valid input");

    assert_profile(&profile, &estimation);

    test_case.assert_output(&public_values);

    profile
}

/// Checks that `profile` splits `estimation` and its peak heap, that a stack ends in the guest
/// `main`, which the guest enters, and that both peaks are nonzero.
pub fn assert_profile(profile: &CostProfile, estimation: &CostEstimation) {
    assert_eq!(
        profile.cost_estimation(),
        *estimation,
        "profile must split the estimated cost"
    );
    assert_eq!(
        stacks(profile, "heap_growth")
            .iter()
            .map(|(_, bytes)| bytes)
            .sum::<u64>(),
        profile.peak_heap_bytes,
        "heap growth must split the peak heap"
    );
    assert!(
        stacks(profile, "calls")
            .iter()
            .any(|(frames, calls)| frames.last() == Some(&"main") && *calls > 0),
        "a stack must end in the guest `main`, which the guest enters"
    );
    assert!(profile.peak_stack_bytes > 0, "peak stack must not be zero");
    assert!(profile.peak_heap_bytes > 0, "peak heap must not be zero");
}

/// Profiles the Sha256 test case of the zkVM-accelerator program, and checks that a stack holds
/// `zkvm_sha256` below `main`.
pub fn run_zkvm_profile_zkvm_interface(zkvm: &impl zkVMProver) {
    let test_case = zkvm_interface::test_case(Accelerator::Sha256);
    let profile = run_zkvm_profile(zkvm, &test_case);
    assert!(profile_stacks(&profile).iter().any(|(frames, _)| {
        frames
            .iter()
            .skip_while(|frame| **frame != "main")
            .any(|frame| *frame == "zkvm_sha256")
    }));
}

/// Profiles `test_case` on `elf` with and without its symbol table, and checks that the profile
/// without symbols has the same cost and peaks and only `[unknown]` frames below the root frames.
pub fn run_zkvm_profile_without_function_symbols(
    zkvm: &mut impl zkVMProver,
    elf: Elf,
    test_case: &impl TestCase,
) {
    let elf_without_symbols = without_symbol_table(&elf);
    zkvm.setup(elf).unwrap();
    let profile = run_zkvm_profile(zkvm, test_case);

    zkvm.setup(elf_without_symbols).unwrap();
    let (public_values, profile_without_symbols) = zkvm
        .profile(&test_case.input())
        .expect("profile should not fail with valid input");
    test_case.assert_output(&public_values);
    assert_eq!(
        profile_without_symbols.cost_estimation(),
        profile.cost_estimation()
    );
    assert_eq!(
        profile_without_symbols.peak_stack_bytes,
        profile.peak_stack_bytes
    );
    assert_eq!(
        profile_without_symbols.peak_heap_bytes,
        profile.peak_heap_bytes
    );
    let root_frames: BTreeSet<&str> = profile_stacks(&profile)
        .into_iter()
        .filter_map(|(frames, _)| frames.into_iter().next())
        .filter(|frame| frame.starts_with('['))
        .chain(["[unknown]"])
        .collect();
    assert!(
        profile_stacks(&profile_without_symbols)
            .iter()
            .all(|(frames, _)| root_frames.contains(frames[0])
                && frames[1..].iter().all(|frame| *frame == "[unknown]")),
        "every frame must be `[unknown]` or a root frame of the profile with symbols"
    );
}

/// `elf` with the type of each symbol table section set to `SHT_NULL`.
fn without_symbol_table(elf: &Elf) -> Elf {
    const SHT_SYMTAB: u32 = 2;
    let mut elf = elf.0.clone();
    let read = |offset: usize, len: usize| {
        elf[offset..offset + len]
            .iter()
            .rev()
            .fold(0, |value, byte| value << 8 | *byte as usize)
    };
    let (section_headers, entry_size, count) = (read(0x28, 8), read(0x3a, 2), read(0x3c, 2));
    let symbol_table_types: Vec<usize> = (0..count)
        .map(|index| section_headers + index * entry_size + 4)
        .filter(|&offset| read(offset, 4) == SHT_SYMTAB as usize)
        .collect();
    assert!(
        !symbol_table_types.is_empty(),
        "ELF must have a symbol table"
    );
    for offset in symbol_table_types {
        elf[offset..offset + 4].fill(0);
    }
    Elf(elf)
}

/// Frames from the root, and the cost, of each stack in `profile`.
fn profile_stacks(profile: &CostProfile) -> Vec<(Vec<&str>, u64)> {
    stacks(profile, "cost")
}

/// Cost of the root frame `name` in `profile`.
pub fn profile_root_cost(profile: &CostProfile, name: &str) -> u64 {
    profile_stacks(profile)
        .into_iter()
        .find(|(frames, _)| frames == &[name])
        .unwrap_or_else(|| panic!("profile must have the root frame `{name}`"))
        .1
}

/// Frames from the root, and the value of the sample type `type_name`, of each stack in `profile`.
fn stacks<'a>(profile: &'a CostProfile, type_name: &str) -> Vec<(Vec<&'a str>, u64)> {
    let pprof = &profile.pprof;
    let index = pprof
        .sample_type
        .iter()
        .position(|sample_type| pprof.string_table[sample_type.r#type as usize] == type_name)
        .unwrap_or_else(|| panic!("profile must have the sample type `{type_name}`"));
    let frame = |location_id: &u64| {
        let function_id = pprof.location[*location_id as usize - 1].line[0].function_id;
        let name = pprof.function[function_id as usize - 1].name;
        pprof.string_table[name as usize].as_str()
    };
    pprof
        .sample
        .iter()
        .map(|sample| {
            let frames = sample.location_id.iter().rev().map(frame).collect();
            (frames, sample.value[index] as u64)
        })
        .collect()
}
