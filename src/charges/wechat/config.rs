use serde::Deserialize;
use std::str::FromStr;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WxPubConfig {
    pub wx_pub_app_id: String,
    pub wx_pub_mch_id: String,
    pub wx_pub_key: String,
    pub wx_pub_client_cert: String,
    pub wx_pub_client_key: String,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum WechatTradeStatus {
    Success,
    Fail,
}

impl FromStr for WechatTradeStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SUCCESS" => Ok(WechatTradeStatus::Success),
            "FAIL" => Ok(WechatTradeStatus::Fail),
            _ => Err(format!("unknown wechat trade status: {}", s)),
        }
    }
}
