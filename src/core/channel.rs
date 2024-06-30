use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Deserialize, Serialize, Debug)]
pub enum PaymentChannel {
    #[serde(rename = "alipay_pc_direct")]
    AlipayPcDirect,
    #[serde(rename = "alipay_wap")]
    AlipayWap,
    #[serde(rename = "wx_pub")]
    WxPub,
    #[serde(rename = "wx_lite")]
    WxLite,
}

impl FromStr for PaymentChannel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = serde_json::Value::String(s.to_string());
        let channel = serde_json::from_value::<PaymentChannel>(val)
            .map_err(|e| format!("error parsing PaymentChannel from string: {:?}", e))?;
        Ok(channel)
    }
}

impl ToString for PaymentChannel {
    fn to_string(&self) -> String {
        let val = serde_json::to_value(self).unwrap();
        val.as_str().unwrap().to_string()
    }
}
