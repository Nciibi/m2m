with open('D:/projects/M2M/src-tauri/src/commands.rs', 'rb') as f:
    lines = f.read().split(b'\n')

def count_real_braces(line_bytes):
    """Count braces excluding those in string literals"""
    opens = closes = 0
    in_str = False
    prev_bs = False  # previous char was backslash
    for b in line_bytes:
        ch = chr(b)
        if in_str:
            if prev_bs:
                prev_bs = False
            elif ch == '\\':
                prev_bs = True
            elif ch == '"':
                in_str = False
        else:
            if ch == '"':
                in_str = True
            elif ch == '{':
                opens += 1
            elif ch == '}':
                closes += 1
    return opens, closes

total_opens = total_closes = 0
for i in range(940, 1045):
    o, c = count_real_braces(lines[i-1])
    total_opens += o
    total_closes += c
    if o != c:
        text = lines[i-1].decode('utf-8', errors='replace').rstrip()
        print(f'L{i}: +{o} -{c} = {o-c:+d}  {text[:80]}')
print(f'\nTotal: +{total_opens} -{total_closes} = {total_opens-total_closes:+d}')
