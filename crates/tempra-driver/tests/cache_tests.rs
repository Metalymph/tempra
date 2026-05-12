use tempra_driver::{run_source_vm, cache};
use std::fs;
use std::path::Path;

#[test]
fn test_cache_lifecycle() {
    let source = "fn main() -> Int64 { return 42 }";
    let hash = cache::compute_source_hash(source);
    let cache_path = cache::get_cache_path(&hash);
    
    // 1. Ensure no cache initially
    if cache_path.exists() {
        fs::remove_file(&cache_path).unwrap();
    }
    
    // 2. First run creates the cache
    let res1 = run_source_vm(source, &[]).expect("First run failed");
    assert_eq!(format!("{:?}", res1), "Int64(42)");
    assert!(cache_path.exists(), "Cache file was not created at {:?}", cache_path);
    
    let metadata_first = fs::metadata(&cache_path).unwrap();
    let mtime_first = metadata_first.modified().unwrap();
    
    // 3. Second run uses the cache (mtime should be same or very close)
    // We wait a bit to ensure if it WAS rewritten, the mtime WOULD change.
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    let res2 = run_source_vm(source, &[]).expect("Second run failed");
    assert_eq!(format!("{:?}", res2), "Int64(42)");
    
    let metadata_second = fs::metadata(&cache_path).unwrap();
    let mtime_second = metadata_second.modified().unwrap();
    assert_eq!(mtime_first, mtime_second, "Cache was unnecessarily rewritten");
    
    // 4. Corrupt the cache
    fs::write(&cache_path, b"CORRUPTED DATA").unwrap();
    
    // 5. Run again - should regenerate
    let res3 = run_source_vm(source, &[]).expect("Run after corruption failed");
    assert_eq!(format!("{:?}", res3), "Int64(42)");
    
    let metadata_third = fs::metadata(&cache_path).unwrap();
    let mtime_third = metadata_third.modified().unwrap();
    assert!(mtime_third > mtime_second, "Cache was not regenerated after corruption");
    
    // 6. Source change creates different file
    let source2 = "fn main() -> Int64 { return 43 }";
    let hash2 = cache::compute_source_hash(source2);
    let cache_path2 = cache::get_cache_path(&hash2);
    
    if cache_path2.exists() {
        fs::remove_file(&cache_path2).unwrap();
    }
    
    let res4 = run_source_vm(source2, &[]).expect("Run with different source failed");
    assert_eq!(format!("{:?}", res4), "Int64(43)");
    assert!(cache_path2.exists());
    assert_ne!(cache_path, cache_path2);
    
    // Cleanup
    fs::remove_file(&cache_path).ok();
    fs::remove_file(&cache_path2).ok();
}
