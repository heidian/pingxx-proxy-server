use rand::Rng;

pub fn generate_id(prefix: &str) -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let mut rng = rand::thread_rng();
    let number: u64 = rng.gen_range(10000000000..100000000000);
    format!("{}{}{}", prefix, timestamp, number)
}
