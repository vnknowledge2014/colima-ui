import os
import re

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
            sig = ""
            start_i = i
            while i < len(lines) and '{' not in lines[i]:
                sig += lines[i]
                i += 1
            if i < len(lines):
                sig += lines[i]
            
            # match signature
            m = re.match(r'(?:#\[\w+\]\s*)*pub\s+(async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(([^)]*)\)\s*->\s*Result<([^,]+(?:,\s*[^>]+)*),\s*String>\s*\{', sig.strip().replace('\n', ' '))
            if m:
                is_async = m.group(1) or ''
                name = m.group(2)
                args_str = m.group(3)
                ret_type = m.group(4)
                
                out.append(f'pub {is_async}fn {name}({args_str}) -> Result<{ret_type}, crate::error::ColimaError> {{\n')
                
                if 'async' in is_async:
                    out.append(f'    async move {{\n')
                else:
                    out.append(f'    (|| -> Result<{ret_type}, String> {{\n')
                
                brace_count = 1
                i += 1
                while i < len(lines) and brace_count > 0:
                    brace_count += lines[i].count('{')
                    brace_count -= lines[i].count('}')
                    if brace_count <= 0:
                        idx = lines[i].rfind('}')
                        before = lines[i][:idx]
                        after = lines[i][idx+1:]
                        out.append(before + '    }\n')
                        
                        if 'async' in is_async:
                            out.append(f'    .await.map_err(|e: String| crate::error::ColimaError::from(e))\n')
                        else:
                            out.append(f'    )().map_err(|e: String| crate::error::ColimaError::from(e))\n')
                            
                        out.append('}' + after)
                        break
                    else:
                        out.append(lines[i])
                    i += 1
            else:
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
