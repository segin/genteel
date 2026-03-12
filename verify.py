with open("src/z80/tests_block.rs", "r") as f:
    text = f.read()

open_braces = text.count('{')
close_braces = text.count('}')
print(f"Open: {open_braces}, Close: {close_braces}")
if open_braces == close_braces:
    print("Braces are balanced.")
else:
    print("Braces are not balanced!")
