import re

with open('src/z80/mod.rs', 'r') as f:
    content = f.read()

# 1. Update step signature
# pub fn step(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface) -> u8
content = content.replace(
    'pub fn step(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface) -> u8',
    'pub fn step(&mut self, bus: &mut (impl MemoryInterface + IoInterface)) -> u8'
)

# 2. Update step implementation calls
# self.execute_x0(memory, io, ...) -> self.execute_x0(bus, ...)
# self.execute_x1(memory, ...) -> self.execute_x1(bus, ...)
# self.execute_x2(memory, ...) -> self.execute_x2(bus, ...)
# self.execute_x3(memory, io, ...) -> self.execute_x3(bus, ...)

content = content.replace('execute_x0(memory, io,', 'execute_x0(bus,')
content = content.replace('execute_x1(memory,', 'execute_x1(bus,')
content = content.replace('execute_x2(memory,', 'execute_x2(bus,')
content = content.replace('execute_x3(memory, io,', 'execute_x3(bus,')
content = content.replace('fetch_byte(memory)', 'fetch_byte(bus)')

# 3. Update execute_x0 signature and usage
# fn execute_x0(&mut self, memory: &mut impl MemoryInterface, _io: &mut impl IoInterface,
content = content.replace(
    'fn execute_x0(&mut self, memory: &mut impl MemoryInterface, _io: &mut impl IoInterface,',
    'fn execute_x0(&mut self, memory: &mut impl MemoryInterface,'
)

# 4. Update execute_x3 signature and usage
# fn execute_x3(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface,
content = content.replace(
    'fn execute_x3(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface,',
    'fn execute_x3(&mut self, bus: &mut (impl MemoryInterface + IoInterface),'
)
# Inside x3, replace memory/io with bus
content = content.replace('execute_x3_jp_out_ex_di_ei(memory, io,', 'execute_x3_jp_out_ex_di_ei(bus,')
content = content.replace('execute_x3_push_call_prefixes(memory, io,', 'execute_x3_push_call_prefixes(bus,')
# Other x3 calls only take memory, pass bus
content = content.replace('execute_x3_ret_cc(memory,', 'execute_x3_ret_cc(bus,')
content = content.replace('execute_x3_pop_ret_exx(memory,', 'execute_x3_pop_ret_exx(bus,')
content = content.replace('execute_x3_jp_cc(memory,', 'execute_x3_jp_cc(bus,')
content = content.replace('execute_x3_call_cc(memory,', 'execute_x3_call_cc(bus,')
content = content.replace('execute_x3_alu_n(memory,', 'execute_x3_alu_n(bus,')
content = content.replace('execute_x3_rst(memory,', 'execute_x3_rst(bus,')

# 5. execute_x3_jp_out_ex_di_ei
content = content.replace(
    'fn execute_x3_jp_out_ex_di_ei(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface,',
    'fn execute_x3_jp_out_ex_di_ei(&mut self, bus: &mut (impl MemoryInterface + IoInterface),'
)
# Body updates
content = content.replace('fetch_word(memory)', 'fetch_word(bus)')
content = content.replace('execute_cb_prefix(memory)', 'execute_cb_prefix(bus)')
content = content.replace('fetch_byte(memory)', 'fetch_byte(bus)')
content = content.replace('write_port(io,', 'write_port(bus,')
content = content.replace('read_port(io,', 'read_port(bus,')
content = content.replace('read_word(memory,', 'read_word(bus,')
content = content.replace('write_word(memory,', 'write_word(bus,')

# 6. execute_x3_push_call_prefixes
content = content.replace(
    'fn execute_x3_push_call_prefixes(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface,',
    'fn execute_x3_push_call_prefixes(&mut self, bus: &mut (impl MemoryInterface + IoInterface),'
)
content = content.replace('push(memory,', 'push(bus,')
# execute_dd_prefix etc calls
content = content.replace('execute_dd_prefix(memory, io)', 'execute_dd_prefix(bus)')
content = content.replace('execute_ed_prefix(memory, io)', 'execute_ed_prefix(bus)')
content = content.replace('execute_fd_prefix(memory, io)', 'execute_fd_prefix(bus)')

# 7. execute_dd_prefix, fd, index - Remove IO
content = content.replace(
    'fn execute_dd_prefix(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface)',
    'fn execute_dd_prefix(&mut self, memory: &mut impl MemoryInterface)'
)
content = content.replace('execute_index_prefix(memory, io,', 'execute_index_prefix(memory,')

content = content.replace(
    'fn execute_fd_prefix(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface)',
    'fn execute_fd_prefix(&mut self, memory: &mut impl MemoryInterface)'
)

content = content.replace(
    'fn execute_index_prefix(&mut self, memory: &mut impl MemoryInterface, _io: &mut impl IoInterface,',
    'fn execute_index_prefix(&mut self, memory: &mut impl MemoryInterface,'
)

# 8. execute_ed_prefix
content = content.replace(
    'fn execute_ed_prefix(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface)',
    'fn execute_ed_prefix(&mut self, bus: &mut (impl MemoryInterface + IoInterface))'
)
# Update body
content = content.replace('read_port(io,', 'read_port(bus,')
content = content.replace('set_reg(memory,', 'set_reg(bus,')
content = content.replace('get_reg(memory,', 'get_reg(bus,')
content = content.replace('write_byte(memory,', 'write_byte(bus,')
content = content.replace('read_byte(memory,', 'read_byte(bus,')
content = content.replace('execute_ldi_ldd(memory,', 'execute_ldi_ldd(bus,')
content = content.replace('execute_cpi_cpd(memory,', 'execute_cpi_cpd(bus,')
content = content.replace('execute_ini_ind(memory, io,', 'execute_ini_ind(bus,')
content = content.replace('execute_outi_outd(memory, io,', 'execute_outi_outd(bus,')

# 9. execute_ini_ind, outi_outd
content = content.replace(
    'fn execute_ini_ind(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface,',
    'fn execute_ini_ind(&mut self, bus: &mut (impl MemoryInterface + IoInterface),'
)
content = content.replace(
    'fn execute_outi_outd(&mut self, memory: &mut impl MemoryInterface, io: &mut impl IoInterface,',
    'fn execute_outi_outd(&mut self, bus: &mut (impl MemoryInterface + IoInterface),'
)

with open('src/z80/mod.rs', 'w') as f:
    f.write(content)
