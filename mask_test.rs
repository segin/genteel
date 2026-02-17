fn main() {
    let row_addr: usize = 0x123456;
    let mask = 0xFFFC;
    println!("0x{:X} & 0x{:X} = 0x{:X}", row_addr, mask, row_addr & mask);
}
