use meow_types::config::{meow_config_dir, meow_keystore_path};
use temp_dir::TempDir;

//
// Configuration tests.
//

#[test]
fn meow_config_dir_with_env_var() {
    let tmp_dir = TempDir::new().unwrap();
    let expected_config_dir = tmp_dir.path().join("config");

    temp_env::with_var("MEOW_CONFIG_DIR", Some(expected_config_dir.clone()), || {
        let config_dir = meow_config_dir().unwrap();

        assert_eq!(config_dir, expected_config_dir);

        assert!(config_dir.exists());
    });
}

#[test]
fn meow_keystore_path_with_env_var() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_config_dir = tmp_dir.path().join("config");
    let expected_keystore_path = tmp_config_dir.join("keystore.json");

    temp_env::with_var("MEOW_CONFIG_DIR", Some(tmp_config_dir.clone()), || {
        let keystore_path = meow_keystore_path().unwrap();

        assert_eq!(keystore_path, expected_keystore_path);

        assert!(!keystore_path.exists());
    });
}
