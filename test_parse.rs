#[derive(Default, Debug)]
struct Config {
    show_help: bool,
    script_path: Option<String>,
    record_path: Option<String>,
    headless: bool,
    headless_frames: Option<u32>,
    screenshot_path: Option<String>,
    gdb_port: Option<u16>,
    dump_audio_path: Option<String>,
    input_mapping: String,
    debug: bool,
    rom_path: Option<String>,
}

fn from_args_old<I>(args: I) -> Config
where
    I: IntoIterator<Item = String>,
{
    let mut config = Config::default();
    let mut iter = args.into_iter().skip(1).peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                config.show_help = true;
            }
            "--script" => {
                config.script_path = iter.next();
            }
            "--record" => {
                config.record_path = iter.next();
            }
            "--headless" => {
                config.headless = true;
                if let Some(next) = iter.peek() {
                    if !next.starts_with('-') {
                        if let Ok(n) = next.parse::<u32>() {
                            config.headless_frames = Some(n);
                            iter.next(); // consume
                        }
                    }
                }
            }
            "--screenshot" => {
                config.screenshot_path = iter.next();
            }
            "--gdb" => {
                let mut port = 1234;
                if let Some(next) = iter.peek() {
                    if !next.starts_with('-') {
                        if let Ok(p) = next.parse() {
                            port = p;
                            iter.next(); // consume it
                        }
                    }
                }
                config.gdb_port = Some(port);
            }
            "--dump-audio" => {
                config.dump_audio_path = iter.next();
            }
            "--input-mapping" => {
                if let Some(mapping_str) = iter.next() {
                    config.input_mapping = mapping_str;
                }
            }
            "--debug" => {
                config.debug = true;
            }
            arg if !arg.starts_with('-') => {
                if let Some(ref mut path) = config.rom_path {
                    path.push(' ');
                    path.push_str(arg);
                } else {
                    config.rom_path = Some(arg.to_string());
                }
            }
            _ => {
                eprintln!("Unknown option: {}", arg);
            }
        }
    }
    config
}

fn from_args_new<I>(args: I) -> Config
where
    I: IntoIterator<Item = String>,
{
    let mut config = Config::default();
    let mut iter = args.into_iter().skip(1);
    let mut current_opt = iter.next();
    while let Some(arg) = current_opt {
        match arg.as_str() {
            "--help" | "-h" => {
                config.show_help = true;
                current_opt = iter.next();
            }
            "--script" => {
                config.script_path = iter.next();
                current_opt = iter.next();
            }
            "--record" => {
                config.record_path = iter.next();
                current_opt = iter.next();
            }
            "--headless" => {
                config.headless = true;
                current_opt = iter.next();
                if let Some(ref next) = current_opt {
                    if !next.starts_with('-') {
                        if let Ok(n) = next.parse::<u32>() {
                            config.headless_frames = Some(n);
                            current_opt = iter.next(); // consume
                        }
                    }
                }
            }
            "--screenshot" => {
                config.screenshot_path = iter.next();
                current_opt = iter.next();
            }
            "--gdb" => {
                let mut port = 1234;
                current_opt = iter.next();
                if let Some(ref next) = current_opt {
                    if !next.starts_with('-') {
                        if let Ok(p) = next.parse() {
                            port = p;
                            current_opt = iter.next(); // consume it
                        }
                    }
                }
                config.gdb_port = Some(port);
            }
            "--dump-audio" => {
                config.dump_audio_path = iter.next();
                current_opt = iter.next();
            }
            "--input-mapping" => {
                if let Some(mapping_str) = iter.next() {
                    config.input_mapping = mapping_str;
                }
                current_opt = iter.next();
            }
            "--debug" => {
                config.debug = true;
                current_opt = iter.next();
            }
            arg if !arg.starts_with('-') => {
                if let Some(ref mut path) = config.rom_path {
                    path.push(' ');
                    path.push_str(arg);
                } else {
                    config.rom_path = Some(arg.to_string());
                }
                current_opt = iter.next();
            }
            _ => {
                eprintln!("Unknown option: {}", arg);
                current_opt = iter.next();
            }
        }
    }
    config
}

fn main() {
    let args: Vec<String> = vec![
        "prog",
        "--headless", "100",
        "--gdb", "5678",
        "--script", "myscript.txt",
        "--debug",
        "rom1.bin",
        "rom2.bin"
    ].into_iter().map(|s| s.to_string()).collect();

    let c1 = from_args_old(args.clone());
    let c2 = from_args_new(args);
    println!("Old: {:?}", c1);
    println!("New: {:?}", c2);
}
