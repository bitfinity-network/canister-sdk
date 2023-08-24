use flate2::read::GzDecoder;
use log::*;
use once_cell::sync::OnceCell;
use std::{
    fs::{create_dir_all, File},
    io::*,
    path::Path,
    time::Duration,
};

pub use ic_test_state_machine_client::*;

pub const IC_STATE_MACHINE_BINARY_HASH: &str = "d33ce10e3896f223045bc44320c794308c32a13e";

/// Returns the path to the ic-test-state-machine binary.
/// If the binary is not present, it downloads it.
/// See: https://github.com/dfinity/test-state-machine-client
///
/// It supports only linux and macos
///
/// The search_path variable is the folder where to search for the binary
/// or to download it if not present
pub fn get_ic_test_state_machine_client_path(search_path: &str) -> String {
    static FILES: OnceCell<String> = OnceCell::new();
    FILES.get_or_init(|| download_binary(search_path)).clone()
}

fn download_binary(base_path: &str) -> String {
    let platform = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "darwin",
        _ => panic!("ic_test_state_machine_client requires linux or macos"),
    };

    let output_file_name = "ic-test-state-machine";
    let gz_file_name = format!("{output_file_name}.gz");
    let download_url = format!("https://download.dfinity.systems/ic/{IC_STATE_MACHINE_BINARY_HASH}/binaries/x86_64-{platform}/{gz_file_name}");

    let dest_path_name = format!("{}/{}", base_path, "ic_test_state_machine");
    let dest_dir_path = Path::new(&dest_path_name);
    let gz_dest_file_path = format!("{}/{}", dest_path_name, gz_file_name);
    let output_dest_file_path = format!("{}/{}", dest_path_name, output_file_name);

    if !Path::new(&output_dest_file_path).exists() {
        // Download file
        {
            info!(
                "ic-test-state-machine binarey not found, downloading binary from: {download_url}"
            );

            let response = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap()
                .get(download_url)
                .send()
                .unwrap();

            create_dir_all(dest_dir_path).unwrap();

            let mut file = match File::create(&gz_dest_file_path) {
                Err(why) => panic!("couldn't create {}", why),
                Ok(file) => file,
            };
            let content = response.bytes().unwrap();
            info!("ic-test-state-machine.gz file length: {}", content.len());
            file.write_all(&content).unwrap();
            file.flush().unwrap();
        }

        // unzip file
        {
            info!(
                "unzip ic-test-state-machine to [{}]",
                dest_dir_path.to_str().unwrap()
            );
            let tar_gz = File::open(gz_dest_file_path).unwrap();
            let mut tar = GzDecoder::new(tar_gz);
            let mut temp = vec![];
            tar.read_to_end(&mut temp).unwrap();

            let mut output = File::create(&output_dest_file_path).unwrap();
            output.write_all(&temp).unwrap();
            output.flush().unwrap();

            #[cfg(target_family = "unix")]
            {
                use std::os::unix::prelude::PermissionsExt;
                let mut perms = std::fs::metadata(&output_dest_file_path)
                    .unwrap()
                    .permissions();
                perms.set_mode(0o770);
                std::fs::set_permissions(&output_dest_file_path, perms).unwrap();
            }
        }
    }
    output_dest_file_path
}

#[test]
fn should_get_ic_test_state_machine_client_path() {
    let path = get_ic_test_state_machine_client_path("../target");
    assert!(Path::new(&path).exists())
}
