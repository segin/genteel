fn main() {
    // If Gilrs::new() fails, we want to know how it fails, but Gilrs::new() usually succeeds on Linux.
    // If we can't reliably make it fail through the GilrsBuilder, we should change Framework's gilrs to Option<Gilrs>
}
