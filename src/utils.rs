use rand::Rng;

pub fn generate_id(prefix: &str) -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let mut rng = rand::thread_rng();
    let number: u64 = rng.gen_range(10000000000..100000000000);
    format!("{}{}{}", prefix, timestamp, number)
}

pub fn notify_url(charge_id: &str) -> String {
    let charge_notify_origin = std::env::var("CHARGE_NOTIFY_ORIGIN").unwrap();
    format!("{}/notify/charges/{}", charge_notify_origin, charge_id)
    // "https://notify.pingxx.com/notify/charges/ch_101240601691280343040013";
}
