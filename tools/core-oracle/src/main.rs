fn main() {
    for hash in [
        odd_core_oracle::fingerprint(),
        odd_core_oracle::integrator_fingerprint(),
    ] {
        for byte in hash {
            print!("{byte:02x}");
        }
        println!();
    }
}
