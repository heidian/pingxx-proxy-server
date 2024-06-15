use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WxPubConfig {
    pub wx_pub_app_id: String,
    pub wx_pub_mch_id: String,
    pub wx_pub_key: String,
    pub wx_pub_client_cert: String,
    pub wx_pub_client_key: String,
}
