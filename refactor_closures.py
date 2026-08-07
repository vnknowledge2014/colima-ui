import os
import re

def parse_args(args_str):
    if not args_str.strip(): return []
    # simple split by comma, ignoring types (which might contain commas if they had generic, but Tauri commands usually don't have complex nested generics in args)
    args = []
    # This is a bit naive but works for standard args like `app: tauri::AppHandle, name: String`
    parts = args_str.split(',')
    for part in parts:
        if ':' in part:
            name = part.split(':')[0].strip()
            # Handle mut
            name = name.replace('mut ', '').strip()
            args.append(name)
    return args

def refactor_file(path):
    with open(path, 'r') as f:
        content = f.read()

    # Find all #[tauri::command] functions
    pattern = r'#\[tauri::command\]\s*(?:#\[\w+\]\s*)*pub\s+(async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(([^)]*)\)\s*->\s*Result<([^,>]+(?:,\s*[^>]+)*),\s*String>\s*\{'
    
    def repl(m):
        is_async = m.group(1) or ''
        name = m.group(2)
        args_str = m.group(3)
        ret_type = m.group(4)
        
        arg_names = parse_args(args_str)
        call_args = ', '.join(arg_names)
        
        await_str = '.await' if is_async.strip() == 'async' else ''
        
        # We replace the original signature with ColimaError, then define inner() with String
        new_sig = f'#[tauri::command]\npub {is_async}fn {name}({args_str}) -> Result<{ret_type}, crate::error::ColimaError> {{\n'
        new_sig += f'    {is_async}fn inner({args_str}) -> Result<{ret_type}, String> {{\n'
        return new_sig

    if re.search(pattern, content):
        print(f"Refactoring {path}")
        
        # We need to find the matching closing brace for the function to append the call to inner()
        # This is hard with regex. Let's do a brace counting.
        
        # Instead, we can just replace all return signatures, but where do we put the closing brace?
        pass

# It's better to use a simple text parsing
def refactor_file_ast(path):
    with open(path, 'r') as f:
        lines = f.readlines()
        
    out = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if '#[tauri::command]' in line:
            out.append(line)
            i += 1
            # Next line(s) should be the function signature
            sig = ""
            start_i = i
            while i < len(lines) and '{' not in lines[i]:
                sig += lines[i]
                i += 1
            sig += lines[i] # the line with '{'
            
            # match signature
            m = re.match(r'(?:#\[\w+\]\s*)*pub\s+(async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(([^)]*)\)\s*->\s*Result<([^,]+(?:,\s*[^>]+)*),\s*String>\s*\{', sig.strip().replace('\n', ' '))
            if m:
                is_async = m.group(1) or ''
                name = m.group(2)
                args_str = m.group(3)
                ret_type = m.group(4)
                
                arg_names = parse_args(args_str)
                call_args = ', '.join(arg_names)
                await_str = '.await' if is_async.strip() == 'async' else ''
                
                out.append(f'pub {is_async}fn {name}({args_str}) -> Result<{ret_type}, crate::error::ColimaError> {{\n')
                out.append(f'    {is_async}fn inner({args_str}) -> Result<{ret_type}, String> {{\n')
                
                # Now we need to read until the closing brace of this function
                brace_count = 1
                i += 1
                while i < len(lines) and brace_count > 0:
                    brace_count += lines[i].count('{')
                    brace_count -= lines[i].count('}')
                    if brace_count == 0:
                        # Found the end of the function!
                        # We append our closing logic BEFORE this brace.
                        # Actually, lines[i] contains the closing brace. We replace it.
                        idx = lines[i].rfind('}')
                        before = lines[i][:idx]
                        after = lines[i][idx+1:]
                        out.append(before + '    }\n')
                        out.append(f'    inner({call_args}){await_str}.map_err(crate::error::ColimaError::from)\n')
                        out.append('}' + after)
                        break
                    else:
                        out.append(lines[i])
                    i += 1
            else:
                # If it doesn't match Result<..., String>, just output it
                out.append(lines[start_i])
                i = start_i
        else:
            out.append(line)
        i += 1
        
    with open(path, 'w') as f:
        f.writelines(out)


for root, _, files in os.walk('src-tauri/src/commands'):
    for file in files:
        if file.endswith('.rs'):
            refactor_file_ast(os.path.join(root, file))
