use std::fs;
use std::path::PathBuf;
use tempra_driver::pipeline::{check_file, diagnose_file};

fn setup_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("tempra_module_tests")
        .join(test_name);
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_module_two_files_success() {
    let dir = setup_temp_dir("two_files_success");

    let math_path = dir.join("math.tmpr");
    fs::write(
        &math_path,
        "pub fn add(a: Int, b: Int) -> Int { return a + b }\n",
    )
    .unwrap();

    let main_path = dir.join("main.tmpr");
    fs::write(
        &main_path,
        "import math\nfn main() {\n    let r = math.add(1, 2);\n    expect(r == 3)\n}\n",
    )
    .unwrap();

    let result = check_file(&main_path);
    assert!(
        result.is_ok(),
        "Expected typecheck success, got: {:?}",
        result
    );
}

#[test]
fn test_module_visibility_failure() {
    let dir = setup_temp_dir("visibility_failure");

    let math_path = dir.join("math.tmpr");
    fs::write(
        &math_path,
        "fn helper() -> Int { return 42 }\npub fn get() -> Int { return helper() }\n",
    )
    .unwrap();

    let main_path = dir.join("main.tmpr");
    fs::write(
        &main_path,
        "import math\nfn main() {\n    let r = math.helper();\n}\n",
    )
    .unwrap();

    let result = check_file(&main_path);
    assert!(result.is_err(), "Expected visibility check to fail");
    let err_msg = format!("{:?}", result.err().unwrap());
    assert!(
        err_msg.contains("is private")
            || err_msg.contains("not visible")
            || err_msg.contains("private"),
        "Unexpected error message: {}",
        err_msg
    );
}

#[test]
fn test_submodule_import_success() {
    let dir = setup_temp_dir("submodule_success");

    let net_dir = dir.join("net");
    fs::create_dir_all(&net_dir).unwrap();

    let http_path = net_dir.join("http.tmpr");
    fs::write(
        &http_path,
        "pub struct Client { pub id: Int }\npub fn get() -> Client { return Client { id: 42 } }\n",
    )
    .unwrap();

    let main_path = dir.join("main.tmpr");
    fs::write(
        &main_path,
        "import net.http\nfn main() {\n    let c = net.http.get();\n    expect(c.id == 42)\n}\n",
    )
    .unwrap();

    let result = check_file(&main_path);
    assert!(
        result.is_ok(),
        "Expected submodule check success, got: {:?}",
        result
    );
}

#[test]
fn test_relative_import_success() {
    let dir = setup_temp_dir("relative_success");

    let utils_dir = dir.join("utils");
    fs::create_dir_all(&utils_dir).unwrap();

    let helper_path = utils_dir.join("helper.tmpr");
    fs::write(
        &helper_path,
        "pub fn greet() -> String { return \"hello\" }\n",
    )
    .unwrap();

    let main_path = utils_dir.join("main.tmpr");
    fs::write(
        &main_path,
        "import .helper\nfn main() {\n    let g = helper.greet();\n    expect(g == \"hello\")\n}\n",
    )
    .unwrap();

    let result = check_file(&main_path);
    assert!(
        result.is_ok(),
        "Expected relative import check success, got: {:?}",
        result
    );
}

#[test]
fn test_import_cycle_detection() {
    let dir = setup_temp_dir("cycle_detection");

    let a_path = dir.join("a.tmpr");
    let b_path = dir.join("b.tmpr");

    fs::write(&a_path, "import b\n").unwrap();
    fs::write(&b_path, "import a\n").unwrap();

    let diags = diagnose_file(&a_path);
    assert!(!diags.is_empty(), "Expected diagnostics for cyclic import");
    let err_msg = diags[0].message.clone();
    assert!(
        err_msg.contains("cyclic import detected"),
        "Expected cyclic import diagnostic, got: {}",
        err_msg
    );
}
