import re

def process_file():
    with open("src/cpu/ops/bits.rs", "r") as f:
        content = f.read()

    # Locate the target function
    target_start = content.find("fn test_exec_shift_comprehensive() {")
    if target_start == -1:
        print("Function not found")
        return

    # Find the end of the struct
    struct_end = content.find("    }", content.find("struct TestCase {")) + 5

    # We want to extract the struct and run_shift_test_cases to module level
    # Find where the struct and runner are
    struct_str = content[content.find("    struct TestCase {"):content.find("    fn run_shift_test_cases")]
    runner_end = content.find("        }", content.find("    fn run_shift_test_cases")) + 9
    runner_str = content[content.find("    fn run_shift_test_cases"):runner_end]

    print(runner_end)

process_file()
