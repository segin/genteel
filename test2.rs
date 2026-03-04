fn main() {
    let name = "sonic.GEN";
    let rom_extensions = [".bin", ".md", ".gen", ".smd", ".32x"];

    let is_rom = rom_extensions.iter().any(|&ext| {
        name.len() >= ext.len() && name[name.len() - ext.len()..].eq_ignore_ascii_case(ext)
    });

    println!("is_rom: {}", is_rom);
}
