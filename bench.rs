use std::time::Instant;

fn main() {
    let args: Vec<String> = vec![
        "prog".to_string(),
        "--headless".to_string(), "100".to_string(),
        "--gdb".to_string(), "5678".to_string(),
        "--script".to_string(), "myscript.txt".to_string(),
        "--debug".to_string(),
        "rom1.bin".to_string(),
        "rom2.bin".to_string()
    ];

    let iters = 1_000_000;

    let start = Instant::now();
    for _ in 0..iters {
        let mut show_help = false;
        let mut script_path = None;
        let mut headless = false;
        let mut headless_frames = None;
        let mut gdb_port = None;
        let mut debug = false;
        let mut rom_path: Option<String> = None;

        let mut iter = args.clone().into_iter().skip(1).peekable();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    show_help = true;
                }
                "--script" => {
                    script_path = iter.next();
                }
                "--headless" => {
                    headless = true;
                    if let Some(next) = iter.peek() {
                        if !next.starts_with('-') {
                            if let Ok(n) = next.parse::<u32>() {
                                headless_frames = Some(n);
                                iter.next(); // consume
                            }
                        }
                    }
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
                    gdb_port = Some(port);
                }
                "--debug" => {
                    debug = true;
                }
                arg if !arg.starts_with('-') => {
                    if let Some(ref mut path) = rom_path {
                        path.push(' ');
                        path.push_str(arg);
                    } else {
                        rom_path = Some(arg.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    let dur_old = start.elapsed();
    println!("Old: {:?}", dur_old);

    let start = Instant::now();
    for _ in 0..iters {
        let mut show_help = false;
        let mut script_path = None;
        let mut headless = false;
        let mut headless_frames = None;
        let mut gdb_port = None;
        let mut debug = false;
        let mut rom_path: Option<String> = None;

        let mut iter = args.clone().into_iter().skip(1);
        let mut current_opt = iter.next();
        while let Some(arg) = current_opt {
            match arg.as_str() {
                "--help" | "-h" => {
                    show_help = true;
                    current_opt = iter.next();
                }
                "--script" => {
                    script_path = iter.next();
                    current_opt = iter.next();
                }
                "--headless" => {
                    headless = true;
                    current_opt = iter.next();
                    if let Some(ref next) = current_opt {
                        if !next.starts_with('-') {
                            if let Ok(n) = next.parse::<u32>() {
                                headless_frames = Some(n);
                                current_opt = iter.next(); // consume
                            }
                        }
                    }
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
                    gdb_port = Some(port);
                }
                "--debug" => {
                    debug = true;
                    current_opt = iter.next();
                }
                arg if !arg.starts_with('-') => {
                    if let Some(ref mut path) = rom_path {
                        path.push(' ');
                        path.push_str(arg);
                    } else {
                        rom_path = Some(arg.to_string());
                    }
                    current_opt = iter.next();
                }
                _ => {
                    current_opt = iter.next();
                }
            }
        }
    }
    let dur_new = start.elapsed();
    println!("New: {:?}", dur_new);
}
