fn main() {
    for byte in odd_core_oracle::fingerprint() {
        print!("{byte:02x}");
    }
    println!();
}
