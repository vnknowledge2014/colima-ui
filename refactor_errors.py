import os
import re

def process_file(path):
    with open(path, 'r') as f:
        content = f.read()

    # Add use crate::error::ColimaError;
    if 'crate::error::ColimaError' not in content:
        # insert after use std::... or at top
        content = '#[allow(unused_imports)]\nuse crate::error::ColimaError;\n' + content

    # Replace Result<T, String> with Result<T, ColimaError>
    # Be careful with things like Result<Vec<String>, String>
    # We will use regex
    
    # Replace Result<..., String> with Result<..., ColimaError>
    content = re.sub(r'Result<([^,]+(?:,\s*[^>]+)*),\s*String>', r'Result<\1, ColimaError>', content)
    
    # Replace Err(format!(...)) with Err(ColimaError::Internal(format!(...)))
    # Wait, simple replace: Err(format!(...)) -> Err(ColimaError::Internal(format!(...)))
    # Regex for Err(format!(...)) is hard due to nested parens.
    # We'll just replace `Err(format!` with `Err(ColimaError::Internal(format!`
    # But then we need an extra `)` at the end. It's safer to use `.map_err(|e| ColimaError::Internal(e.to_string()))`
    
    with open(path, 'w') as f:
        f.write(content)

for root, _, files in os.walk('src-tauri/src/commands'):
    for file in files:
        if file.endswith('.rs'):
            process_file(os.path.join(root, file))

for root, _, files in os.walk('src-tauri/src/routes'):
    for file in files:
        if file.endswith('.rs'):
            process_file(os.path.join(root, file))

process_file('src-tauri/src/helpers.rs')
process_file('src-tauri/src/api_server.rs')
