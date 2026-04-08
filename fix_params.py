import os

def fix_render_ops():
    path = "src/vdp/render.rs"
    with open(path, "r") as f:
        src = f.read()

    # Wait, the error `cannot find value line_buf in this scope` was in `render_plane` line 431.
    # Ah, the argument to `render_tile` is `line_buf`, but `render_plane` doesn't have it?
    # Let's fix RenderOps implementations:

    src = src.replace(
"""    fn render_plane(
        &mut self,""",
"""    fn render_plane(
        &mut self,
        line_buf: &mut [u8; 320],""")

    with open(path, "w") as f:
        f.write(src)

if __name__ == '__main__':
    fix_render_ops()
