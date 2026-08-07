import os
import re

def refactor_file(path):
    with open(path, 'r') as f:
        content = f.read()
        
    # Replace Result<T, String> with Result<T, crate::error::ColimaError>
    content = re.sub(r'Result<([^,>]+(?:,\s*[^>]+)*),\s*String>', r'Result<\1, crate::error::ColimaError>', content)
    
    # Replace return Err(format!(...));
    # We'll use a loop to match format! and replace it
    # We can match `Err(format!` and replace with `Err(crate::error::ColimaError::Internal(format!`
    # But we have to balance parentheses. It's easier to just use `map_err(|e| crate::error::ColimaError::Internal(e.to_string()))`
    
    # Let's replace map_err(|e| e.to_string())
    content = re.sub(r'map_err\(\|e\| e\.to_string\(\)\)', r'map_err(|e| crate::error::ColimaError::Internal(e.to_string()))', content)
    
    # Let's replace map_err(|e| format!(...))
    # map_err(|e| format!("foo", e)) -> map_err(|e| crate::error::ColimaError::Internal(format!("foo", e)))
    content = re.sub(r'map_err\(\|(.*?)\|\s*format!\(', r'map_err(|\1| crate::error::ColimaError::Internal(format!(', content)
    
    # Let's replace Err("...".to_string()) -> Err(crate::error::ColimaError::Internal("...".to_string()))
    content = re.sub(r'Err\("([^"]+)"\.to_string\(\)\)', r'Err(crate::error::ColimaError::Internal("\1".to_string()))', content)
    
    # Let's replace Err(format!(...))
    content = re.sub(r'Err\(format!\(', r'Err(crate::error::ColimaError::Internal(format!(', content)
    
    # Let's replace return Err(e.to_string())
    content = re.sub(r'Err\(e\.to_string\(\)\)', r'Err(crate::error::ColimaError::Internal(e.to_string()))', content)
    
    with open(path, 'w') as f:
        f.write(content)

for root, _, files in os.walk('src-tauri/src/commands'):
    for file in files:
        if file.endswith('.rs'):
            refactor_file(os.path.join(root, file))

refactor_file('src-tauri/src/helpers.rs')
refactor_file('src-tauri/src/api_server.rs')
