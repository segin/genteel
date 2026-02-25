use genteel::input::{FrameInput, InputManager, InputScript};
use std::borrow::Cow;
use std::time::Instant;

fn main() {
    let mut manager = InputManager::new();

    // Better way to build script:
    manager.start_recording();
    for i in 0..1_000_000 {
        let mut input = FrameInput::default();
        if i % 1000 == 0 {
            input.command = Some(format!("COMMAND {}", i));
        }
        input.p1.a = i % 2 == 0;
        manager.record(input);
        manager.advance_frame();
    }
    let script: InputScript = manager.stop_recording();

    manager.reset();
    manager.set_script(script);

    let start = Instant::now();
    for _ in 0..1_000_000 {
        let _input: Cow<'_, FrameInput> = manager.advance_frame();
    }
    let duration = start.elapsed();

    println!("Processed 1,000,000 frames in {:?}", duration);
    println!("Average time per frame: {:?}", duration / 1_000_000);
}
