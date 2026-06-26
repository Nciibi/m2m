with open('D:/projects/M2M/src-tauri/src/commands.rs', 'rb') as f:
    lines = f.read().split(b'\n')

def count_real_braces(line_bytes):
    opens = closes = 0
    in_str = False
    for b in line_bytes:
        ch = chr(b)
        if ch == '"':
            in_str = not in_str
        elif not in_str:
            if ch == '{':
                opens += 1
            elif ch == '}':
                closes += 1
    return opens, closes

# Track depth from file beginning
depth = 0
for i in range(1, 758):
    o, c = count_real_braces(lines[i-1])
    depth += o - c

print(f'Depth before spawn_receive_loop: {depth}')

# Track through the function
for i in range(758, 1156):
    o, c = count_real_braces(lines[i-1])
    depth += o - c

# Show depth at function closure points
check_lines = {
    763: 'fn {',
    764: 'async move {',
    765: 'loop {',
    784: 'match {',
    833: 'FileTransferReq start',
    888: 'FileTransferReq end',
    889: 'FileTransferChunk start',
    939: 'FileTransferChunk end',
    940: 'FileTransferComplete start',
    1044: 'FileTransferComplete end',
    1045: 'FileTransferAccept start',
    1152: 'match }',
    1153: 'loop }',
    1154: 'async }',
    1155: 'fn }',
}

for ln, label in sorted(check_lines.items()):
    o, c = count_real_braces(lines[ln-1])
    print(f'L{ln:4d}: depth={depth:2d}  {label}')
    # Don't modify depth — we want the depth AFTER processing the line
    # which is already done in the loop above
