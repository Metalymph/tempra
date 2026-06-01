import re

with open("crates/tempra-driver/src/pipeline.rs", "r") as f:
    content = f.read()

# Add missing arguments to lower_function calls
content = re.sub(r'lower_function\(([^,]+),\s*([^,]+),\s*([^,]+),\s*None,\s*\)', r'lower_function(\1, \2, \3, None, None)', content)
content = re.sub(r'lower_function\(([^,]+),\s*([^,]+),\s*([^,]+),\s*Some\(([^)]+)\),\s*\)', r'lower_function(\1, \2, \3, Some(\4), None)', content)
content = content.replace("                    &mut module_builder,\n                    None,\n                ) {", "                    &mut module_builder,\n                    None,\n                    None,\n                ) {")
content = content.replace("                        &mut module_builder,\n                        Some(target_ty.clone()),\n                    ) {", "                        &mut module_builder,\n                        Some(target_ty.clone()),\n                        None,\n                    ) {")
content = content.replace("                &mut module_builder,\n                None,\n            ) {", "                &mut module_builder,\n                None,\n                None,\n            ) {")

with open("crates/tempra-driver/src/pipeline.rs", "w") as f:
    f.write(content)
