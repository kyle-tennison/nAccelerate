//! The nTop password lives in the system keychain, keyed by nTop username.

const SERVICE: &str = "nAccelerate";

pub fn save(user: &str, password: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, user)
        .and_then(|e| e.set_password(password))
        .map_err(|e| e.to_string())
}

pub fn load(user: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, user).and_then(|e| e.get_password()).ok()
}
