import re

with open("crates/tempra-typeck/src/check_expr.rs", "r") as f:
    content = f.read()

def extract_block(name, start_marker, end_marker):
    start = content.find(start_marker)
    if start == -1: raise Exception(f"Start not found: {start_marker}")
    end = content.find(end_marker, start)
    if end == -1: raise Exception(f"End not found: {end_marker}")
    end = end + len(end_marker)
    return content[start:end], start, end

# Block 1: validate_pattern_type
b1_start = "fn validate_pattern_type("
b1_end = "        HirPattern::EnumConstructor(_) => return None,\n    }\n}"
block1, s1, e1 = extract_block("validate_pattern_type", b1_start, b1_end)

# Block 2: extract_pattern_bindings
b2_start = "fn extract_pattern_bindings("
b2_end = "        HirPattern::Or(_, alternatives) => {\n            if let Some(first) = alternatives.get_mut(0) {\n                extract_pattern_bindings(ctx, val_ty, first, out);\n            }\n        }\n    }\n}"
block2, s2, e2 = extract_block("extract_pattern_bindings", b2_start, b2_end)

# Block 3: compare_bindings
b3_start = "fn compare_bindings("
b3_end = "    }\n    true\n}"
block3, s3, e3 = extract_block("compare_bindings", b3_start, b3_end)

# Block 4: patterns_structurally_identical
b4_start = "fn patterns_structurally_identical("
b4_end = "        _ => false,\n    }\n}"
block4, s4, e4 = extract_block("patterns_structurally_identical", b4_start, b4_end)

# Block 5: exhaustiveness
b5_start = "// ─── Exhaustiveness Groundwork v3 ─────────────────────────────────────────────"
b5_end = "        Domain::Tuple(_) | Domain::Struct(_) | Domain::Unknown => None,\n    }\n}"
block5, s5, e5 = extract_block("exhaustiveness", b5_start, b5_end)

pattern_rs = f"""//! Shared pattern semantics.

use crate::context::TypeContext;
use crate::types::Type;
use tempra_hir::*;

pub {block1.replace('fn validate_pattern_type', 'fn validate_pattern_against_type').replace('validate_pattern_type', 'validate_pattern_against_type').replace('extract_pattern_bindings', 'extract_pattern_bindings').replace('compare_bindings', 'compare_bindings').replace('patterns_structurally_identical', 'patterns_structurally_identical')}

pub {block2.replace('fn extract_pattern_bindings', 'fn extract_pattern_bindings')}

pub {block3.replace('fn compare_bindings', 'fn compare_bindings')}

pub {block4.replace('fn patterns_structurally_identical', 'fn patterns_structurally_identical')}

{block5.replace('enum MatchCoverageKey', 'pub enum MatchCoverageKey').replace('enum Space', 'pub enum Space').replace('enum Domain', 'pub enum Domain').replace('fn pattern_to_structural_space', 'pub fn pattern_structural_space').replace('fn pattern_to_coverage_space', 'pub fn pattern_coverage_space').replace('pattern_to_structural_space', 'pattern_structural_space').replace('fn domain_of_type', 'pub fn domain_of_type').replace('fn cases_of_domain', 'pub fn cases_of_domain')}

pub fn pattern_allowed_in_is(ctx: &mut TypeContext, pattern: &HirPattern) -> bool {{
    match pattern {{
        HirPattern::Or(span, _) => {{
            ctx.error("or-patterns are not allowed in is", *span);
            false
        }}
        HirPattern::Wildcard(span) => {{
            ctx.error("wildcard patterns are not allowed in is", *span);
            false
        }}
        HirPattern::Identifier(span, _) => {{
            ctx.error("naked binding is not allowed in is", *span);
            false
        }}
        _ => true,
    }}
}}
"""

with open("crates/tempra-typeck/src/pattern.rs", "w") as f:
    f.write(pattern_rs)

# Modify check_expr.rs
# Replace the removed blocks with empty string
# Also replace their usages

content = content[:s5] + content[e5:]
content = content[:s4] + content[e4:]
content = content[:s3] + content[e3:]
content = content[:s2] + content[e2:]
content = content[:s1] + content[e1:]

content = content.replace("validate_pattern_type", "crate::pattern::validate_pattern_against_type")
content = content.replace("extract_pattern_bindings", "crate::pattern::extract_pattern_bindings")
content = content.replace("pattern_to_structural_space", "crate::pattern::pattern_structural_space")
content = content.replace("pattern_to_coverage_space", "crate::pattern::pattern_coverage_space")
content = content.replace("domain_of_type", "crate::pattern::domain_of_type")
content = content.replace("cases_of_domain", "crate::pattern::cases_of_domain")
content = content.replace("MatchCoverageKey", "crate::pattern::MatchCoverageKey")
content = content.replace("Space", "crate::pattern::Space")
content = content.replace("Domain", "crate::pattern::Domain")

# Let's fix the `check_is_pattern` to use `pattern_allowed_in_is`
is_pattern_search = """    // Explicit Restrictions for 'is'
    match &is_pattern.pattern {
        HirPattern::Or(span, _) => {
            ctx.error("or-patterns are not allowed in is", *span);
        }
        HirPattern::Wildcard(span) => {
            ctx.error("wildcard patterns are not allowed in is", *span);
        }
        HirPattern::Identifier(span, _) => {
            ctx.error("naked binding is not allowed in is", *span);
        }
        _ => {}
    }"""

is_pattern_replace = """    if !crate::pattern::pattern_allowed_in_is(ctx, &is_pattern.pattern) {
        // Restrictions already logged
    }"""

content = content.replace(is_pattern_search, is_pattern_replace)

with open("crates/tempra-typeck/src/check_expr.rs", "w") as f:
    f.write(content)

# Modify lib.rs
with open("crates/tempra-typeck/src/lib.rs", "r") as f:
    lib_content = f.read()

lib_content = lib_content.replace("pub mod check_expr;", "pub mod check_expr;\npub mod pattern;")

with open("crates/tempra-typeck/src/lib.rs", "w") as f:
    f.write(lib_content)
