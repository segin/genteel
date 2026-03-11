1. **Define `OpArgs` struct:**
   - In `src/z80/mod.rs`, define a new struct `OpArgs` containing `opcode`, `x`, `y`, `z`, `p`, `q`.
   - Add a `new` method to initialize it from a `u8` opcode.

2. **Update `GeneralOps` trait in `src/z80/op_general.rs`:**
   - Modify the signatures of `execute_x0`, `execute_x1`, `execute_x2`, and `execute_x3` to take `args: OpArgs` instead of multiple parameters.

3. **Update `GeneralOps` implementation in `src/z80/op_general.rs`:**
   - Update `execute_x0`, `execute_x1`, `execute_x2`, and `execute_x3` implementations.
   - Replace uses of `y`, `z`, `p`, `q`, etc., with `args.y`, `args.z`, `args.p`, `args.q`.

4. **Update `execute` method in `src/z80/mod.rs`:**
   - Replace the local variable declarations of `x`, `y`, `z`, `p`, `q` with `let args = OpArgs::new(opcode);`.
   - Pass `args` to `execute_x0`, `execute_x1`, `execute_x2`, `execute_x3` within the `match args.x` block.

5. **Test and Verify:**
   - Run `cargo fmt` and `cargo check --lib`.
   - Run `cargo test --lib z80` to ensure no functionality is broken.
   - Run the pre commit instructions step to ensure proper testing, verification, review, and reflection are done.
