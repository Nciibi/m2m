with open('D:/projects/M2M/src-tauri/src/commands.rs', 'rb') as f:
    lines = f.read().split(b'\n')

def count_braces(line_bytes):
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

depth = 0
for i in range(1, 940):
    o, c = count_braces(lines[i-1])
    depth += o - c

print(f'Depth at L940 (start of FileTransferComplete): {depth}')

for i in range(940, 1045):
    o, c = count_braces(lines[i-1])
    depth += o - c
    text = lines[i-1].decode('utf-8', errors='replace').rstrip()[:80]
    if o != c:
        print(f'L{i:4d}: depth after={depth:2d}  +{o} -{c}  {text}')
    elif i in (940, 941, 1044):
        print(f'L{i:4d}: depth after={depth:2d}  +{o} -{c}  {text}')

print(f'\nDepth at L1044 (end): {depth}')
print(f'Expected: {depth} should be 4 (= base before arm)')
