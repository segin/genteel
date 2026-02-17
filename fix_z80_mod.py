import re

with open('src/z80/mod.rs', 'r') as f:
    content = f.read()

# Replace argument name 'memory' with 'bus' in signatures
# Pattern: memory: &mut impl MemoryInterface
content = content.replace('memory: &mut impl MemoryInterface', 'bus: &mut impl MemoryInterface')

# Also handle cases where I might have left 'io' but it should be 'bus' or removed?
# execute_ini_ind takes 'bus: &mut (impl MemoryInterface + IoInterface)' (handled in previous step)

# Now verify usages.
# fetch_byte(bus) -> Correct if arg is bus.
# read_port(io, ...) -> read_port(bus, ...)
content = content.replace('read_port(io,', 'read_port(bus,')
content = content.replace('write_port(io,', 'write_port(bus,')

# Some signatures might still have 'io'.
# fn execute_dd_prefix(bus: &mut impl MemoryInterface, io: &mut impl IoInterface)
# This logic was: dd_prefix calls index_prefix(bus, io).
# But index_prefix signature was changed to remove io.
# So dd_prefix shouldn't take io.
# My previous script tried to remove io from dd_prefix signature.
# Check if it succeeded.

# Also I need to make sure I didn't break 'read_byte(bus, ...)' calls if signature expects 'bus'.

with open('src/z80/mod.rs', 'w') as f:
    f.write(content)
