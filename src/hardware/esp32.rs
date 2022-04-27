use esp_idf_sys::esp_efuse_mac_get_default;

pub fn get_unique_id() -> String {
    let mut mac: [u8; 6] = [0; 6];
    unsafe {
        let ptr = &mut mac as *mut u8;
        esp_efuse_mac_get_default(ptr);
    }
    hex::encode(mac)
}
