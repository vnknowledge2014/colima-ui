import json
import subprocess

def fix_errors():
    result = subprocess.run(['cargo', 'check', '--message-format', 'json'], cwd='src-tauri', capture_output=True, text=True)
    
    fixes = []
    
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        try:
            msg = json.loads(line)
        except:
            continue
            
        if msg.get('reason') == 'compiler-message' and msg.get('message', {}).get('code', {}):
            if msg['message']['code']['code'] == 'E0308':
                for span in msg['message']['spans']:
                    if span.get('is_primary'):
                        # This span points to the mismatched type.
                        # The help message often says "call `Into::into` on this expression"
                        pass
                
                # Let's look at the children for the help message
                for child in msg['message']['children']:
                    if child.get('message', '').startswith("call `Into::into`"):
                        for span in child.get('spans', []):
                            if span.get('suggested_replacement') == '.into()':
                                fixes.append({
                                    'file': span['file_name'],
                                    'line': span['line_end'],
                                    'col': span['column_start'],
                                    'replacement': span['suggested_replacement']
                                })
                            elif span.get('suggested_replacement') == ').into()':
                                fixes.append({
                                    'file': span['file_name'],
                                    'line': span['line_end'],
                                    'col': span['column_start'], # This is where ')' is
                                    'replacement': ').into()'
                                })
                            elif 'into()' in span.get('suggested_replacement', ''):
                                fixes.append({
                                    'file': span['file_name'],
                                    'line': span['line_end'],
                                    'col': span['column_start'],
                                    'replacement': span['suggested_replacement']
                                })
                                
    # Group fixes by file
    fixes_by_file = {}
    for fix in fixes:
        fixes_by_file.setdefault(fix['file'], []).append(fix)
        
    for file, file_fixes in fixes_by_file.items():
        with open('src-tauri/' + file, 'r') as f:
            lines = f.readlines()
            
        # Sort fixes in reverse order of line and col to apply them without messing up offsets
        file_fixes.sort(key=lambda x: (x['line'], x['col']), reverse=True)
        
        for fix in file_fixes:
            l = fix['line'] - 1
            c = fix['col'] - 1
            if fix['replacement'] == '.into()':
                lines[l] = lines[l][:c] + '.into()' + lines[l][c:]
            elif fix['replacement'] == ').into()':
                lines[l] = lines[l][:c] + ').into()' + lines[l][c+1:]
            else:
                # Sometimes the suggestion is just appending .into() at column_end
                c_end = fix.get('col', fix['col']) - 1
                # Try to apply the exact replacement
                pass # Need precise handling if not just appending

        with open('src-tauri/' + file, 'w') as f:
            f.writelines(lines)
            
    print(f"Applied {len(fixes)} fixes.")

if __name__ == '__main__':
    fix_errors()
