import re

with open('src/z80/mod.rs', 'r') as f:
    content = f.read()

# Replace usage of 'memory' with 'bus' in function bodies
# We are looking for 'memory.read_byte', 'memory.write_byte', etc.
content = content.replace('memory.read_byte', 'bus.read_byte')
content = content.replace('memory.write_byte', 'bus.write_byte')
content = content.replace('memory.read_word', 'bus.read_word')
content = content.replace('memory.write_word', 'bus.write_word')
content = content.replace('memory.read_long', 'bus.read_long')
content = content.replace('memory.write_long', 'bus.write_long')

# Also passed as argument to other functions
# e.g. self.pop(memory) -> self.pop(bus)
# self.fetch_byte(memory) -> self.fetch_byte(bus)
# But I already did fetch_byte(memory) -> fetch_byte(bus) in update_z80_mod.py?
# Yes, but maybe I missed some or reverted?

content = content.replace('fetch_byte(memory)', 'fetch_byte(bus)')
content = content.replace('fetch_word(memory)', 'fetch_word(bus)')
content = content.replace('read_byte(memory', 'read_byte(bus')
content = content.replace('write_byte(memory', 'write_byte(bus')
content = content.replace('read_word(memory', 'read_word(bus')
content = content.replace('write_word(memory', 'write_word(bus')
content = content.replace('push(memory', 'push(bus')
content = content.replace('pop(memory', 'pop(bus')
content = content.replace('get_reg(memory', 'get_reg(bus')
content = content.replace('set_reg(memory', 'set_reg(bus')

# Calls to execute_*
content = content.replace('execute_x0_control_misc(memory', 'execute_x0_control_misc(bus')
content = content.replace('execute_x0_load_add_hl(memory', 'execute_x0_load_add_hl(bus')
content = content.replace('execute_x0_load_indirect(memory', 'execute_x0_load_indirect(bus')
content = content.replace('execute_x0_inc_r(memory', 'execute_x0_inc_r(bus')
content = content.replace('execute_x0_dec_r(memory', 'execute_x0_dec_r(bus')
content = content.replace('execute_x0_ld_r_n(memory', 'execute_x0_ld_r_n(bus')

content = content.replace('execute_indexed_cb(memory', 'execute_indexed_cb(bus')
content = content.replace('execute_index_prefix(memory', 'execute_index_prefix(bus')

content = content.replace('execute_ldi_ldd(memory', 'execute_ldi_ldd(bus')
content = content.replace('execute_cpi_cpd(memory', 'execute_cpi_cpd(bus')
content = content.replace('execute_ini_ind(memory', 'execute_ini_ind(bus')
content = content.replace('execute_outi_outd(memory', 'execute_outi_outd(bus')

with open('src/z80/mod.rs', 'w') as f:
    f.write(content)
