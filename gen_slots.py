def generate_slots(pattern):
    res = []
    # simplify the pattern strings
    for char in pattern:
        if char == ' ':
            continue
        elif char == '~':
            res.append(True)
        else:
            res.append(False)
    return res

h40 = "Hssss AsaaBsbb" + (" A~aaBSbb A~aaBSbb A~aaBSbb AraaBSbb" * 5) + " ~~ " + "s"*23 + " ~ " + "s"*11
h32 = "Hssss AsaaBsbb" + (" A~aaBSbb A~aaBSbb A~aaBSbb AraaBSbb" * 4) + " ~~ " + "s"*13 + " ~ " + "s"*13 + " ~"

h40_slots = generate_slots(h40)
h32_slots = generate_slots(h32)

print(f"pub const H40_EXTERNAL_SLOTS: [bool; 210] = {h40_slots};".replace("True", "true").replace("False", "false"))
print(f"pub const H32_EXTERNAL_SLOTS: [bool; 171] = {h32_slots};".replace("True", "true").replace("False", "false"))
