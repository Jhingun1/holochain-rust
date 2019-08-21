use error::DefaultResult;
use holochain_common::paths::keys_directory;
use holochain_conductor_api::{
    key_loaders::mock_passphrase_manager,
    keystore::{Keystore, PRIMARY_KEYBUNDLE_ID},
};
use holochain_core_types::error::HcResult;
use holochain_dpki::{
    seed::{RootSeed, Seed, SeedType, TypedSeed},
    utils::SeedContext,
    CONTEXT_SIZE,
};
use std::{
    fs::create_dir_all,
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use util::get_secure_string_double_check;

/// If a root seed is passed then decode it from BIP39 (TODO: Also suppot base64)
/// If not then securely prompt the user for the seed then attempt to decode
/// TODO: Decrypt the root seed using the passphrase (currently it is unencrypted)
fn get_root_seed(
    root_seed: Option<String>,
    _passphrase: &String,
    quiet: bool,
) -> HcResult<RootSeed> {
    let seed_string = root_seed.unwrap_or_else(|| {
        get_secure_string_double_check("Root Seed", quiet).expect("Coult not retrieve root seed")
    });

    // try and parse the seed from string
    let root_seed = Seed::new_with_mnemonic(seed_string, SeedType::Root)?;

    match root_seed.into_typed()? {
        TypedSeed::Root(inner_root_seed) => Ok(inner_root_seed),
        _ => unreachable!(),
    }
}

pub fn keygen(
    path: Option<PathBuf>,
    passphrase: Option<String>,
    quiet: bool,
    root_seed: Option<String>,
    device_derivation_index: Option<u64>,
) -> DefaultResult<()> {
    let passphrase = passphrase.unwrap_or_else(|| {
        if !quiet {
            println!(
                "
This will create a new agent keystore and populate it with an agent keybundle
containing a public and a private key, for signing and encryption by the agent.
This keybundle will be stored encrypted by passphrase within the keystore file.
The passphrase is securing the keys and will be needed, together with the file,
in order to use the key.
Please enter a secret passphrase below. You will have to enter it again
when unlocking the keybundle to use within a Holochain conductor."
            );
            print!("Passphrase: ");
            io::stdout().flush().expect("Could not flush stdout");
        }
        get_secure_string_double_check("Passphrase", quiet).expect("Could not retrieve passphrase")
    });

    if !quiet {
        println!("Generating keystore (this will take a few moments)...");
    }

    let (keystore, pub_key) = if device_derivation_index.is_some() {
        println!("This keystore is to be generated from a DPKI root seed.");
        let mut root_seed = get_root_seed(root_seed, &passphrase, quiet)?;
        let device_derivation_index = device_derivation_index.expect(
            "Device derivation context is ensured to be set together with root_seed in main.rs",
        );

        let mut context_array: [u8; CONTEXT_SIZE] = Default::default();
        let context_string = String::from("HCDEVICE");
        let context_slice = context_string.as_bytes();
        context_array.copy_from_slice(context_slice);
        let seed_context = SeedContext::new(context_array);

        let mut keystore = Keystore::new(mock_passphrase_manager(passphrase), None)?;
        let device_seed = root_seed.generate_device_seed(&seed_context, device_derivation_index)?;
        keystore.add("device_seed", Arc::new(Mutex::new(device_seed.into())))?;
        let (pub_key, _) = keystore.add_keybundle_from_seed("device_seed", PRIMARY_KEYBUNDLE_ID)?;
        (keystore, pub_key)
    } else {
        Keystore::new_standalone(mock_passphrase_manager(passphrase), None)?
    };

    let path = if None == path {
        let p = keys_directory();
        create_dir_all(p.clone())?;
        p.join(pub_key.clone())
    } else {
        path.unwrap()
    };

    keystore.save(path.clone())?;
    let path_str = path.to_str().unwrap();

    if quiet {
        println!("{}", pub_key);
        println!("{}", path_str);
    } else {
        println!("");
        println!("Succesfully created new agent keystore.");
        println!("");
        println!("Public address: {}", pub_key);
        println!("Keystore written to: {}", path_str);
        println!("");
        println!("You can set this file in a conductor config as keystore_file for an agent.");
    }
    Ok(())
}

#[cfg(test)]
pub mod test {
    use super::*;
    use cli::dpki_init::dpki_init;
    use holochain_conductor_api::{
        key_loaders::mock_passphrase_manager,
        keystore::{Keystore, PRIMARY_KEYBUNDLE_ID},
    };
    use std::{fs::remove_file, path::PathBuf};

    #[test]
    fn keygen_roundtrip_no_dpki() {
        let path = PathBuf::new().join("test.key");
        let passphrase = String::from("secret");

        keygen(
            Some(path.clone()),
            Some(passphrase.clone()),
            true,
            None,
            None,
        )
        .expect("Keygen should work");

        let mut keystore =
            Keystore::new_from_file(path.clone(), mock_passphrase_manager(passphrase), None)
                .unwrap();

        let keybundle = keystore.get_keybundle(PRIMARY_KEYBUNDLE_ID);

        assert!(keybundle.is_ok());

        let _ = remove_file(path);
    }

    #[test]
    fn keygen_roundtrip_with_dpki() {
        let path = PathBuf::new().join("test_dpki.key");
        let passphrase = String::from("secret_dpki");

        let mnemonic = dpki_init(Some("dummy passphrase".to_string()))
            .expect("Could not generate root seed mneomonic");

        keygen(
            Some(path.clone()),
            Some(passphrase.clone()),
            true,
            Some(mnemonic),
            Some(1),
        )
        .expect("Keygen should work");

        let mut keystore =
            Keystore::new_from_file(path.clone(), mock_passphrase_manager(passphrase), None)
                .unwrap();

        let keybundle = keystore.get_keybundle(PRIMARY_KEYBUNDLE_ID);

        assert!(keybundle.is_ok());

        let _ = remove_file(path);
    }
}
