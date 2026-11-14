fn main() {
    for hash in [
        odd_core_oracle::fingerprint(),
        odd_core_oracle::integrator_fingerprint(),
        odd_core_oracle::blended_integrator_fingerprint(),
        odd_core_oracle::analytical_integrator_fingerprint(),
        odd_core_oracle::maneuver_fingerprint(),
    ] {
        for byte in hash {
            print!("{byte:02x}");
        }
        println!();
    }
}
