with open("src/gui.rs", "r") as f:
    content = f.read()

content = content.replace("let debug_info = collect_debug_info(emulator, force_red, pixels.frame_mut());", "let debug_info = collect_debug_info(&mut emulator, force_red, pixels.frame_mut());")

with open("src/gui.rs", "w") as f:
    f.write(content)
