use rand::random;

fn main() {
    loop {
        println!("id: {:X}", random::<u64>());
    }
}
