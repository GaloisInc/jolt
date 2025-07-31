use crate::field::JoltField;
use crate::host;
use crate::jolt::vm::rv32i_vm::RV32IJoltVM;
use crate::jolt::vm::{Jolt, JoltProverPreprocessing, JoltVerifierPreprocessing};
use crate::poly::commitment::commitment_scheme::{CommitmentScheme, StreamingCommitmentScheme};
use crate::poly::commitment::dory::{DoryCommitmentScheme as Dory, DoryGlobals};
use crate::poly::commitment::hyperkzg::HyperKZG;
use crate::subprotocols::twist::{TwistAlgorithm, TwistProof};
use crate::utils::math::Math;
use crate::utils::transcript::{KeccakTranscript, Transcript};
use crate::zkvm::JoltVerifierPreprocessing;
use crate::zkvm::{Jolt, JoltRV32IM};
use ark_bn254::Fr;
use ark_std::test_rng;
use rand_core::RngCore;
use rand_distr::{Distribution, Zipf};

#[derive(Debug, Copy, Clone, clap::ValueEnum)]
pub enum BenchType {
    Fibonacci,
    Sha2,
    Sha3,
    Sha2Chain,
    Shout,
    Twist,
}

pub fn benchmarks(bench_type: BenchType) -> Vec<(tracing::Span, Box<dyn FnOnce()>)> {
    match bench_type {
        BenchType::Sha2 => sha2(),
        BenchType::Sha3 => sha3(),
        BenchType::Sha2Chain => sha2_chain(),
        BenchType::Fibonacci => fibonacci(),
        BenchType::Shout => shout(),
        BenchType::Twist => twist::<Fr, KeccakTranscript>(),
    }
}

fn shout() -> Vec<(tracing::Span, Box<dyn FnOnce()>)> {
    todo!()
}

fn twist<F, ProofTranscript>() -> Vec<(tracing::Span, Box<dyn FnOnce()>)>
where
    F: JoltField,
    ProofTranscript: Transcript,
{
    let small_value_lookup_tables = F::compute_lookup_tables();
    F::initialize_lookup_tables(small_value_lookup_tables);

    let mut tasks = Vec::new();

    const K: usize = 1 << 10;
    const T: usize = 1 << 20;
    const ZIPF_S: f64 = 0.0;
    let zipf = Zipf::new(K as u64, ZIPF_S).unwrap();

    let mut rng = test_rng();

    let mut registers = [0u32; K];
    let mut read_addresses: Vec<usize> = Vec::with_capacity(T);
    let mut read_values: Vec<u32> = Vec::with_capacity(T);
    let mut write_addresses: Vec<usize> = Vec::with_capacity(T);
    let mut write_values: Vec<u32> = Vec::with_capacity(T);
    let mut write_increments: Vec<i64> = Vec::with_capacity(T);
    for _ in 0..T {
        // Random read register
        let read_address = zipf.sample(&mut rng) as usize - 1;
        // Random write register
        let write_address = zipf.sample(&mut rng) as usize - 1;
        read_addresses.push(read_address);
        write_addresses.push(write_address);
        // Read the value currently in the read register
        read_values.push(registers[read_address]);
        // Random write value
        let write_value = rng.next_u32();
        write_values.push(write_value);
        // The increment is the difference between the new value and the old value
        let write_increment = (write_value as i64) - (registers[write_address] as i64);
        write_increments.push(write_increment);
        // Write the new value to the write register
        registers[write_address] = write_value;
    }

    let mut prover_transcript = ProofTranscript::new(b"test_transcript");
    let r: Vec<F> = prover_transcript.challenge_vector(K.log_2());
    let r_prime: Vec<F> = prover_transcript.challenge_vector(T.log_2());

    let task = move || {
        let _proof = TwistProof::prove(
            read_addresses,
            read_values,
            write_addresses,
            write_values,
            write_increments,
            r.clone(),
            r_prime.clone(),
            &mut prover_transcript,
            TwistAlgorithm::Local,
        );
    };

    tasks.push((
        tracing::info_span!("Twist d=1"),
        Box::new(task) as Box<dyn FnOnce()>,
    ));

    tasks
}

fn fibonacci() -> Vec<(tracing::Span, Box<dyn FnOnce()>)> {
    prove_example("fibonacci-guest", postcard::to_stdvec(&400000u32).unwrap())
}

fn sha2() -> Vec<(tracing::Span, Box<dyn FnOnce()>)> {
    prove_example("sha2-guest", postcard::to_stdvec(&vec![5u8; 2048]).unwrap())
}

<<<<<<< HEAD
fn sha3() -> Vec<(tracing::Span, Box<dyn FnOnce()>)> {
    prove_example("sha3-guest", postcard::to_stdvec(&vec![5u8; 2048]).unwrap())
=======
fn sha2<F, PCS, ProofTranscript>() -> Vec<(tracing::Span, Box<dyn FnOnce()>)>
where
    F: JoltField,
    PCS: StreamingCommitmentScheme<Field = F>,
    ProofTranscript: Transcript,
{
    prove_example_dag::<Vec<u8>, PCS, F, ProofTranscript>("sha2-guest", &vec![5u8; 10000])
>>>>>>> a7c429ed (Rebase fixes)
}

fn sha2_chain() -> Vec<(tracing::Span, Box<dyn FnOnce()>)> {
    let mut inputs = vec![];
    inputs.append(&mut postcard::to_stdvec(&[5u8; 32]).unwrap());
    inputs.append(&mut postcard::to_stdvec(&1000u32).unwrap());
    prove_example("sha2-chain-guest", inputs)
}

fn prove_example(
    example_name: &str,
    serialized_input: Vec<u8>,
) -> Vec<(tracing::Span, Box<dyn FnOnce()>)> {
    let mut tasks = Vec::new();
    let mut program = host::Program::new(example_name);
    let (bytecode, init_memory_state, _) = program.decode();
    let (_, _, program_io) = program.trace(&serialized_input);

    let task = move || {
<<<<<<< HEAD
        let preprocessing = JoltRV32IM::prover_preprocess(
=======
        let (_lazy_trace, trace, final_memory_state, io_device) = program.trace(&inputs);
        let (bytecode, init_memory_state) = program.decode();

        let preprocessing: JoltProverPreprocessing<F, PCS> = RV32IJoltVM::prover_preprocess(
>>>>>>> 188e56ba (Pass through LazyTraceIterator)
            bytecode.clone(),
            program_io.memory_layout.clone(),
            init_memory_state,
            1 << 24,
        );

        let (jolt_proof, program_io, _) =
            JoltRV32IM::prove(&preprocessing, &mut program, &serialized_input);

        let verifier_preprocessing = JoltVerifierPreprocessing::from(&preprocessing);
        let verification_result =
            JoltRV32IM::verify(&verifier_preprocessing, jolt_proof, program_io, None);
        assert!(
            verification_result.is_ok(),
            "Verification failed with error: {:?}",
            verification_result.err()
        );
    };

    tasks.push((
        tracing::info_span!("Example_E2E"),
        Box::new(task) as Box<dyn FnOnce()>,
    ));

    tasks
}

fn prove_example_dag<T: Serialize, PCS, F, ProofTranscript>(
    example_name: &str,
    input: &T,
) -> Vec<(tracing::Span, Box<dyn FnOnce()>)>
where
    F: JoltField,
    PCS: StreamingCommitmentScheme<Field = F>,
    ProofTranscript: Transcript,
{
    let mut tasks = Vec::new();
    let mut program = host::Program::new(example_name);
    let inputs = postcard::to_stdvec(input).unwrap();

    let task = move || {
        let (lazy_trace, mut trace, final_memory_state, mut io_device) = program.trace(&inputs);
        let (bytecode, init_memory_state) = program.decode();

        let preprocessing: JoltProverPreprocessing<F, PCS> = RV32IJoltVM::prover_preprocess(
            bytecode.clone(),
            io_device.memory_layout.clone(),
            init_memory_state,
            1 << 18,
            1 << 18,
            1 << 24,
        );

        let trace_length = trace.len();
        let padded_trace_length = trace_length.next_power_of_two();
        trace.resize(padded_trace_length, RV32IMCycle::NoOp);

        // Truncate trailing zeros on device outputs
        io_device.outputs.truncate(
            io_device
                .outputs
                .iter()
                .rposition(|&b| b != 0)
                .map_or(0, |pos| pos + 1),
        );

        // Initialize Dory globals
        // let _guard = DoryGlobals::initialize(1 << 18, 1 << 20);

        // Create state manager components
        let prover_accumulator_pre_wrap =
            crate::poly::opening_proof::ProverOpeningAccumulator::<F>::new();
        let prover_accumulator = Rc::new(RefCell::new(prover_accumulator_pre_wrap));
        let prover_transcript = Rc::new(RefCell::new(ProofTranscript::new(b"Jolt")));
        let proofs = Rc::new(RefCell::new(HashMap::new()));
        let commitments = Rc::new(RefCell::new(None));

        // Create prover state manager
        let mut prover_state_manager = state_manager::StateManager::new_prover(
            prover_accumulator,
            prover_transcript.clone(),
            proofs.clone(),
            commitments.clone(),
        );
        prover_state_manager.set_prover_data(
            &preprocessing,
            lazy_trace,
            trace.clone(),
            io_device.clone(),
            final_memory_state.clone(),
        );

        // We only need the prover state manager for benchmarking
        let verifier_accumulator_pre_wrap =
            crate::poly::opening_proof::VerifierOpeningAccumulator::<F>::new();
        let verifier_accumulator = Rc::new(RefCell::new(verifier_accumulator_pre_wrap));
        let verifier_transcript = Rc::new(RefCell::new(ProofTranscript::new(b"Jolt")));
        let verifier_state_manager = state_manager::StateManager::new_verifier(
            verifier_accumulator,
            verifier_transcript.clone(),
            proofs,
            commitments,
        );

        let mut dag = jolt_dag::JoltDAG::new(prover_state_manager, verifier_state_manager);

        // Only run the prover
        if let Err(e) = dag.prove() {
            panic!("DAG prove failed: {e}");
        }
    };

    tasks.push((
        tracing::info_span!("DAG_Prover_Only"),
        Box::new(task) as Box<dyn FnOnce()>,
    ));

    tasks
}

fn sha2chain<F, PCS, ProofTranscript>() -> Vec<(tracing::Span, Box<dyn FnOnce()>)>
where
    F: JoltField,
    PCS: StreamingCommitmentScheme<Field = F>,
    ProofTranscript: Transcript,
{
    let mut tasks = Vec::new();
    let mut program = host::Program::new("sha2-chain-guest");

    let mut inputs = vec![];
    inputs.append(&mut postcard::to_stdvec(&[5u8; 32]).unwrap());
    inputs.append(&mut postcard::to_stdvec(&1000u32).unwrap());

    let task = move || {
        let (lazy_trace, mut trace, final_memory_state, mut io_device) = program.trace(&inputs);
        let (bytecode, init_memory_state) = program.decode();

        let preprocessing: JoltProverPreprocessing<F, PCS> = RV32IJoltVM::prover_preprocess(
            bytecode.clone(),
            io_device.memory_layout.clone(),
            init_memory_state,
            1 << 18,
            1 << 18,
            1 << 25,
        );

        // Setup trace length and padding (similar to DAG test)
        let trace_length = trace.len();
        let padded_trace_length = trace_length.next_power_of_two();
        trace.resize(padded_trace_length, RV32IMCycle::NoOp);

        // Truncate trailing zeros on device outputs
        io_device.outputs.truncate(
            io_device
                .outputs
                .iter()
                .rposition(|&b| b != 0)
                .map_or(0, |pos| pos + 1),
        );

        // Initialize Dory globals
        // let _guard = DoryGlobals::initialize(1 << 18, 1 << 20);

        // Create state manager components
        let prover_accumulator_pre_wrap =
            crate::poly::opening_proof::ProverOpeningAccumulator::<F>::new();
        let prover_accumulator = Rc::new(RefCell::new(prover_accumulator_pre_wrap));
        let prover_transcript = Rc::new(RefCell::new(ProofTranscript::new(b"Jolt")));
        let proofs = Rc::new(RefCell::new(HashMap::new()));
        let commitments = Rc::new(RefCell::new(None));

        // Create prover state manager
        let mut prover_state_manager = state_manager::StateManager::new_prover(
            prover_accumulator,
            prover_transcript.clone(),
            proofs.clone(),
            commitments.clone(),
        );
        prover_state_manager.set_prover_data(
            &preprocessing,
            lazy_trace,
            trace.clone(),
            io_device.clone(),
            final_memory_state.clone(),
        );

        // We only need the prover state manager for benchmarking
        let verifier_accumulator_pre_wrap =
            crate::poly::opening_proof::VerifierOpeningAccumulator::<F>::new();
        let verifier_accumulator = Rc::new(RefCell::new(verifier_accumulator_pre_wrap));
        let verifier_transcript = Rc::new(RefCell::new(ProofTranscript::new(b"Jolt")));
        let verifier_state_manager = state_manager::StateManager::new_verifier(
            verifier_accumulator,
            verifier_transcript.clone(),
            proofs,
            commitments,
        );

        let mut dag = jolt_dag::JoltDAG::new(prover_state_manager, verifier_state_manager);

        // Only run the prover
        if let Err(e) = dag.prove() {
            panic!("DAG prove failed: {e}");
        }
    };

    tasks.push((
        tracing::info_span!("DAG_Prover_Only"),
        Box::new(task) as Box<dyn FnOnce()>,
    ));

    tasks
}
