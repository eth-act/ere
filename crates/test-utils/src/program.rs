use ere_io::Io;
use ere_platform_trait::Platform;

pub mod basic;

/// Program that can be ran given [`Platform`] implementation.
pub trait Program {
    type Io: Io;

    fn compute(input: <Self::Io as Io>::Input) -> <Self::Io as Io>::Output;

    fn run<P: Platform>() {
        let input_bytes = P::read_whole_input();
        let input = Self::Io::deserialize_input(&input_bytes).unwrap();
        let output = Self::compute(input);
        let output_bytes = Self::Io::serialize_output(&output).unwrap();
        P::write_whole_output(&output_bytes);
    }
}
