
    #[test]
    fn test_step_frame_perf() {
        let mut emulator = Emulator::new();
        // Whitelist current directory for ROM loading if needed, though step_frame doesn't load ROM
        emulator.add_allowed_path(".").unwrap();

        let mut input = crate::input::FrameInput::default();
        // Set some input to simulate real usage
        input.p1.a = true;

        let start = std::time::Instant::now();
        for _ in 0..1000 {
            emulator.step_frame(Some(input.clone()));
        }
        let elapsed = start.elapsed();
        println!("1000 frames with clone: {:?}", elapsed);
    }
