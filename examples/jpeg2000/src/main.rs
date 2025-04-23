use guest::MyArray;
use std::time::Instant;

pub fn main() {
    // Prove/verify jpeg2000:
    let target_dir = "/tmp/jolt-guest-targets";

    let program_jpeg2000 = guest::compile_jpeg2000(target_dir);
    let prover_jpeg2000_preprocessing = guest::preprocess_prover_jpeg2000(&program_jpeg2000);
    let verifier_jpeg2000_preprocessing = guest::preprocess_verifier_jpeg2000(&program_jpeg2000);
    let prove_jpeg2000 =
        guest::build_prover_jpeg2000(program_jpeg2000, prover_jpeg2000_preprocessing);
    let verify_jpeg2000 = guest::build_verifier_jpeg2000(verifier_jpeg2000_preprocessing);

    // prove jpeg2000
    let now = Instant::now();
    //let jpeg2000_data = include_bytes!("/Users/benoit/SIEVE/CERRIDWEN/ACTECP/images/ex.jp2");
    //let jpeg2000_data =
    //    include_bytes!("/Users/benoit/SIEVE/CERRIDWEN/ACTECP/images/Small-fire45KB.jp2");
    let jpeg2000_data = include_bytes!("/Users/benoit/SIEVE/CERRIDWEN/ACTECP/images/relax.jp2");
    let image_len = jpeg2000_data.len();
    let image = MyArray::new(jpeg2000_data);

    let (output, proof) = prove_jpeg2000(image.clone(), image_len);
    println!("Prover jpeg2000 runtime: {} s", now.elapsed().as_secs_f64());

    let now = Instant::now();
    let is_valid = verify_jpeg2000(image, image_len, output, proof);
    println!(
        "Verifier jpeg2000 runtime: {} s",
        now.elapsed().as_secs_f64()
    );
    println!("valid: {}", is_valid);
    /*
    let now = Instant::now();
    let input = 19;
    let (output, proof) = prove_collatz_single(input);
    println!("Prover runtime: {} s", now.elapsed().as_secs_f64());
    let is_valid = verify_collatz_single(input, output, proof);

    println!("output: {}", output);
    println!("valid: {}", is_valid);

    // Prove/verify convergence for a range of numbers:
    let program = guest::compile_collatz_convergence_range(target_dir);

    let prover_preprocessing = guest::preprocess_prover_collatz_convergence_range(&program);
    let verifier_preprocessing = guest::preprocess_verifier_collatz_convergence_range(&program);

    let prove_collatz_convergence =
        guest::build_prover_collatz_convergence_range(program, prover_preprocessing);
    let verify_collatz_convergence =
        guest::build_verifier_collatz_convergence_range(verifier_preprocessing);

    // https://www.reddit.com/r/compsci/comments/gk9x6g/collatz_conjecture_news_recently_i_managed_to/
    let start: u128 = 1 << 68;
    let now = Instant::now();
    let (output, proof) = prove_collatz_convergence(start, start + 100);
    println!("Prover runtime: {} s", now.elapsed().as_secs_f64());
    let is_valid = verify_collatz_convergence(start, start + 100, output, proof);

    println!("output: {}", output);
    println!("valid: {}", is_valid);
    */
}
