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

# Track depth, show at checkpoints
depth = 0
checkpoints = {763, 764, 765, 784, 833, 888, 889, 939, 940, 1044, 1045, 1152, 1153, 1154, 1155}

for i in range(1, 1156):
    o, c = count_braces(lines[i-1])
    depth += o - c
    if i in checkpoints:
        text = lines[i-1].decode('utf-8', errors='replace').rstrip()[:60]
        print(f'L{i:4d}: depth={depth:2d}  {text}')
