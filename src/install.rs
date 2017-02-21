#[cfg(windows)] use winreg::RegKey;
#[cfg(windows)] use winreg::enums::*;
#[cfg(windows)] use std::path::Path;

use errors::*;
use std::path::PathBuf;

#[cfg(windows)]
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

#[cfg(windows)]
pub fn install_handler(scheme: &str, command: &str, cfg: &PathBuf) -> Result<()> {
    let hcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let path = Path::new(scheme);
    let key = hcr.create_subkey(&path)?;
    key.set_value("URL Protocol", &"")?;
    key.set_value("url-handler", &VERSION.unwrap_or("unknown"))?;

    let path = path.join("shell").join("open").join("command");
    let key = hcr.create_subkey(&path)?;
    let cmd = &*format!(r#""{}" "%1" "--config" "{}" "#, command, cfg.to_string_lossy()); // FIXME
    key.set_value("", &cmd)?;

    Ok(())
}

#[cfg(windows)]
pub fn list_all() -> Vec<String> {
    let mut v = vec![];
    let hcr = RegKey::predef(HKEY_CLASSES_ROOT);
    for i in hcr.enum_keys()
        .map(|x| x.unwrap_or("".into()))
        .filter(|x| !x.starts_with(".")) {
        match hcr.open_subkey(&i) {
            Ok(k) => {
                let r : String = k.get_value("url-handler").unwrap_or("".into());
                if r != "" {
                    v.push(i);
                }
            },
            Err(_) => continue
        }
    }
    v
}

#[cfg(windows)]
pub fn uninstall_all() -> Result<()> {
    let v = list_all();
    let hcr = RegKey::predef(HKEY_CLASSES_ROOT);
    for i in v {
        hcr.delete_subkey_all(&i)?;
    }
    Ok(())
}

#[cfg(unix)]
#[allow(unused_variables)]
pub fn install_handler(scheme: &str, command: &str) -> Result<()> {
    Err("install not implemented".into())
}

#[cfg(unix)]
pub fn list_all() -> Vec<String> {
    vec![]
}

#[cfg(unix)]
pub fn uninstall_all() -> Result<()> {
    Err("install not implemented".into())
}